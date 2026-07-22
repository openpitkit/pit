# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://openpit.dev and the OWNERS file for details.

"""Cross-platform helpers for local just recipes."""

from __future__ import annotations

import argparse
import atexit
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from collections.abc import Callable, Iterable, Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILD_MODES = {"debug", "release"}
NODE_DIST_URL = "https://nodejs.org/dist"
_WINDOWS_CGO_COMPILER_COMMANDS: dict[tuple[str, ...], str] = {}
_WINDOWS_CGO_WRAPPER_DIRS: set[Path] = set()


def _cleanup_windows_cgo_wrapper_dirs() -> None:
    for wrapper_dir in list(_WINDOWS_CGO_WRAPPER_DIRS):
        shutil.rmtree(wrapper_dir, ignore_errors=True)
    _WINDOWS_CGO_WRAPPER_DIRS.clear()
    _WINDOWS_CGO_COMPILER_COMMANDS.clear()


atexit.register(_cleanup_windows_cgo_wrapper_dirs)


def run(
    args: Sequence[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    command = list(args)
    if not command:
        raise SystemExit("missing executable: empty command")
    try:
        return subprocess.run(
            command,
            cwd=cwd or ROOT,
            env=env,
            check=True,
            text=True,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.PIPE if capture else None,
        )
    except FileNotFoundError as exc:
        executable = command[0] if command else str(exc.filename)
        raise SystemExit(
            f"missing executable: {executable}; install it or add it to PATH"
        ) from None


def run_shell(command: Sequence[str], *, cwd: Path, env: dict[str, str]) -> None:
    run(command, cwd=cwd, env=env)


def chunks(items: Sequence[Path], size: int) -> Iterable[Sequence[Path]]:
    for index in range(0, len(items), size):
        yield items[index : index + size]


def is_windows() -> bool:
    return os.name == "nt"


def build_mode(value: str) -> str:
    if value not in BUILD_MODES:
        modes = ", ".join(sorted(BUILD_MODES))
        raise SystemExit(f"unsupported build mode {value!r}; expected one of: {modes}")
    return value


def build_profile(mode: str) -> str:
    return build_mode(mode)


def cargo_release_args(mode: str) -> list[str]:
    return ["--release"] if build_mode(mode) == "release" else []


def cmake_build_type(mode: str) -> str:
    return "Release" if build_mode(mode) == "release" else "Debug"


def cpp_build_dir(kind: str, mode: str) -> str:
    profile = build_profile(mode)
    if kind == "binding":
        return f"bindings/cpp/build-{profile}"
    if kind == "examples":
        return f"examples/cpp/build-{profile}"
    raise SystemExit(f"unsupported C++ build kind: {kind}")


def host_arch() -> str:
    machine = platform.machine().lower()
    if machine in {"amd64", "x86_64"}:
        return "amd64"
    if machine in {"aarch64", "arm64"}:
        return "arm64"
    return machine


def node_platform() -> str:
    platforms = {
        "Darwin": "darwin",
        "Linux": "linux",
        "Windows": "win",
    }
    try:
        return platforms[platform.system()]
    except KeyError:
        raise SystemExit(
            f"unsupported platform for project-local Node.js: {platform.system()}"
        ) from None


def node_arch() -> str:
    machine = platform.machine().lower()
    architectures = {
        "amd64": "x64",
        "x86_64": "x64",
        "aarch64": "arm64",
        "arm64": "arm64",
    }
    try:
        return architectures[machine]
    except KeyError:
        raise SystemExit(
            f"unsupported architecture for project-local Node.js: {machine}"
        ) from None


def node_archive_name(version: str) -> str:
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise SystemExit(f"invalid CI_NODE version: {version!r}")
    platform_name = node_platform()
    extension = "zip" if platform_name == "win" else "tar.xz"
    return f"node-v{version}-{platform_name}-{node_arch()}.{extension}"


def node_bin_dir(node_dir: Path) -> Path:
    return node_dir if is_windows() else node_dir / "bin"


def node_executable(node_dir: Path) -> Path:
    name = "node.exe" if is_windows() else "node"
    return node_bin_dir(node_dir) / name


def node_runtime_version(node_dir: Path) -> str | None:
    executable = node_executable(node_dir)
    if not executable.is_file():
        return None
    try:
        result = subprocess.run(
            [str(executable), "--version"],
            check=True,
            text=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    match = re.fullmatch(r"v?(\d+\.\d+\.\d+)", result.stdout.strip())
    return match.group(1) if match is not None else None


def sha256_bytes(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download(url: str, destination: Path) -> None:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "openpit-project-node-bootstrap"},
    )
    try:
        with (
            urllib.request.urlopen(request, timeout=60) as response,
            destination.open("wb") as output,
        ):
            shutil.copyfileobj(response, output)
    except urllib.error.URLError as exc:
        raise SystemExit(f"could not download {url}: {exc.reason}") from None


def node_archive_sha256(checksums: Path, archive_name: str) -> str:
    for line in checksums.read_text(encoding="utf-8").splitlines():
        digest, separator, filename = line.partition("  ")
        if separator and filename == archive_name:
            if re.fullmatch(r"[0-9a-f]{64}", digest):
                return digest
            break
    raise SystemExit(f"checksum for {archive_name} was not found")


def extract_node_archive(archive: Path, destination: Path) -> Path:
    destination.mkdir()
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            package.extractall(destination)
    else:
        with tarfile.open(archive, "r:xz") as package:
            if sys.version_info >= (3, 12):
                package.extractall(destination, filter="data")
            else:
                package.extractall(destination)
    roots = [path for path in destination.iterdir() if path.is_dir()]
    if len(roots) != 1:
        raise SystemExit(f"unexpected Node.js archive layout in {archive.name}")
    return roots[0]


def install_node_runtime(node_dir: Path, version: str) -> None:
    archive_name = node_archive_name(version)
    release_url = f"{NODE_DIST_URL}/v{version}"
    node_dir.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="openpit-node-", dir=node_dir.parent
    ) as temp_dir_string:
        temp_dir = Path(temp_dir_string)
        archive = temp_dir / archive_name
        checksums = temp_dir / "SHASUMS256.txt"
        download(f"{release_url}/{archive_name}", archive)
        download(f"{release_url}/SHASUMS256.txt", checksums)
        expected_sha256 = node_archive_sha256(checksums, archive_name)
        actual_sha256 = sha256_bytes(archive)
        if actual_sha256 != expected_sha256:
            raise SystemExit(
                f"checksum mismatch for {archive_name}: "
                f"expected {expected_sha256}, got {actual_sha256}"
            )
        extracted = extract_node_archive(archive, temp_dir / "extracted")
        backup = node_dir.with_name(f"{node_dir.name}.previous")
        if backup.exists():
            shutil.rmtree(backup)
        if node_dir.exists():
            node_dir.replace(backup)
        try:
            extracted.replace(node_dir)
            installed = node_runtime_version(node_dir)
            if installed != version:
                raise SystemExit(
                    f"project-local Node.js installation expected {version}, got "
                    f"{installed or 'an unreadable runtime'}"
                )
        except BaseException:
            if node_dir.exists():
                shutil.rmtree(node_dir)
            if backup.exists():
                backup.replace(node_dir)
            raise
        else:
            if backup.exists():
                shutil.rmtree(backup)


def command_ensure_node(args: argparse.Namespace) -> None:
    expected = ci_version("CI_NODE")
    node_dir = Path(args.node_dir)
    installed = node_runtime_version(node_dir)
    if installed == expected:
        print(f"Node.js {expected}: project-local runtime is current")
        return
    action = "installing" if installed is None else f"upgrading from {installed}"
    print(f"Node.js {expected}: {action} project-local runtime")
    install_node_runtime(node_dir, expected)
    print(f"Node.js {expected}: project-local runtime installed")


def rust_host_triple(command_prefix: Sequence[str] = ()) -> str:
    result = run([*command_prefix, "rustc", "-vV"], capture=True)
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ").strip()
    raise SystemExit("could not determine Rust host target")


def windows_target_triple() -> str:
    return os.environ.get("PIT_WINDOWS_TARGET", "x86_64-pc-windows-msvc")


def rustup_toolchain_installed(toolchain: str) -> bool:
    if shutil.which("rustup") is None:
        return False
    result = run(["rustup", "toolchain", "list"], capture=True)
    return any(line.split()[0] == toolchain for line in result.stdout.splitlines())


def windows_rustup_prefix(target: str) -> list[str]:
    toolchain = os.environ.get("PIT_WINDOWS_RUSTUP_TOOLCHAIN") or f"stable-{target}"
    if not rustup_toolchain_installed(toolchain):
        return []
    prefix = ["rustup", "run", toolchain]
    if rust_host_triple(prefix) != target:
        return []
    return prefix


def cargo_command() -> list[str]:
    if not is_windows():
        return ["cargo"]
    target = windows_target_triple()
    if rust_host_triple() == target:
        return ["cargo"]
    prefix = windows_rustup_prefix(target)
    if prefix:
        return [*prefix, "cargo"]
    return ["cargo"]


def prepend_path(env: dict[str, str], directory: Path) -> None:
    path_key = "Path" if is_windows() else "PATH"
    current = env.get(path_key) or env.get("PATH") or ""
    env[path_key] = str(directory) + os.pathsep + current


def configure_windows_temp_env(env: dict[str, str]) -> None:
    if not is_windows():
        return
    temp_dir = ROOT / ".tmp" / "subprocess-temp"
    temp_dir.mkdir(parents=True, exist_ok=True)
    env["TEMP"] = str(temp_dir)
    env["TMP"] = str(temp_dir)


def configure_windows_rust_env(env: dict[str, str], cargo: Sequence[str]) -> bool:
    rustup_fallback = (
        is_windows()
        and len(cargo) >= 4
        and cargo[:2] == ["rustup", "run"]
        and cargo[-1] == "cargo"
    )
    if not rustup_fallback:
        return False
    rustup = shutil.which("rustup")
    if rustup is not None:
        prepend_path(env, Path(rustup).parent)
    env["RUSTUP_TOOLCHAIN"] = cargo[2]
    env["CARGO_TARGET_DIR"] = str(ROOT / "target" / f"rustup-{windows_target_triple()}")
    return True


def env_get(env: dict[str, str], name: str) -> str:
    value = env.get(name)
    if value is not None:
        return value
    name_lower = name.lower()
    for key, current in env.items():
        if key.lower() == name_lower:
            return current
    return ""


def cargo_target_linker_env_var(target: str) -> str:
    normalized = target.upper().replace("-", "_")
    return f"CARGO_TARGET_{normalized}_LINKER"


def msvc_target_arch_dir(target: str) -> str:
    if target.startswith("x86_64-"):
        return "x64"
    if target.startswith("aarch64-"):
        return "arm64"
    if target.startswith("i686-"):
        return "x86"
    raise SystemExit(f"unsupported MSVC target for linker lookup: {target}")


def msvc_host_arch_dir() -> str:
    if host_arch() == "arm64":
        return "Hostarm64"
    return "Hostx64"


def msvc_linker_candidates(env: dict[str, str], target: str) -> list[Path]:
    target_arch = msvc_target_arch_dir(target)
    host_arch_dir = msvc_host_arch_dir()
    candidates: list[Path] = []

    tools_dir = env_get(env, "VCToolsInstallDir")
    if tools_dir:
        candidates.append(
            Path(tools_dir) / "bin" / host_arch_dir / target_arch / "link.exe"
        )

    roots = [
        env_get(env, "VCINSTALLDIR"),
        (
            str(Path(env_get(env, "VSINSTALLDIR")) / "VC")
            if env_get(env, "VSINSTALLDIR")
            else ""
        ),
    ]
    for root in roots:
        if not root:
            continue
        tools_root = Path(root) / "Tools" / "MSVC"
        if not tools_root.is_dir():
            continue
        for version in sorted(tools_root.iterdir(), reverse=True):
            candidates.append(
                version / "bin" / host_arch_dir / target_arch / "link.exe"
            )

    return candidates


def git_for_windows_linker(path: str | None) -> bool:
    if not path:
        return False
    normalized = str(path).replace("\\", "/").lower()
    return "/git/usr/bin/link.exe" in normalized


def configure_windows_msvc_linker_env(env: dict[str, str]) -> None:
    if not is_windows():
        return
    target = windows_target_triple()
    if not target.endswith("-pc-windows-msvc"):
        return
    linker_var = cargo_target_linker_env_var(target)
    if env_get(env, linker_var):
        return
    for candidate in msvc_linker_candidates(env, target):
        if candidate.is_file():
            env[linker_var] = str(candidate)
            return
    env_path = env_get(env, "PATH")
    if git_for_windows_linker(shutil.which("link", path=env_path or None)):
        raise SystemExit(
            "MSVC link.exe was not found, and Git for Windows link.exe is first "
            "in PATH. Run from a Visual Studio developer shell or configure "
            "ilammy/msvc-dev-cmd before Windows MSVC cargo recipes."
        )


def ensure_windows_target_installed(command_prefix: Sequence[str] = ()) -> None:
    if not is_windows():
        return
    target = windows_target_triple()
    if rust_host_triple(command_prefix) == target:
        return
    libdir = Path(
        run(
            [*command_prefix, "rustc", "--print", "target-libdir", "--target", target],
            capture=True,
        ).stdout.strip()
    )
    if libdir.is_dir():
        return
    raise SystemExit(
        f"Rust target {target} is required for Windows just recipes. "
        f"Install the matching CI/CD toolchain, fix PATH, or set "
        f"PIT_WINDOWS_RUSTUP_TOOLCHAIN=stable-{target}"
    )


def cmake_platform_args() -> list[str]:
    if not is_windows():
        return []
    target = windows_target_triple()
    if target.startswith("i686-"):
        return ["-A", "Win32"]
    if target.startswith("x86_64-"):
        return ["-A", "x64"]
    if target.startswith("aarch64-"):
        return ["-A", "ARM64"]
    return []


def reset_cmake_build_if_platform_changed(build: Path, platform_name: str) -> None:
    cache = build / "CMakeCache.txt"
    if not cache.is_file():
        return
    cached_platform = ""
    for line in cache.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("CMAKE_GENERATOR_PLATFORM:INTERNAL="):
            cached_platform = line.partition("=")[2]
            break
    if cached_platform != platform_name:
        shutil.rmtree(build, onerror=remove_readonly)


def remove_readonly(function: Callable[[str], object], path: str, _: object) -> None:
    os.chmod(path, stat.S_IWRITE)
    function(path)


def prepare_cmake_build_dir(build: str, args: Sequence[str]) -> None:
    if not is_windows() or len(args) < 2 or args[0] != "-A":
        return
    reset_cmake_build_if_platform_changed(ROOT / build, args[1])


def runtime_library_name() -> str:
    if sys.platform == "darwin":
        return "libopenpit_ffi.dylib"
    if is_windows():
        return "openpit_ffi.dll"
    if sys.platform.startswith("linux"):
        return "libopenpit_ffi.so"
    raise SystemExit(f"unsupported OS for pit-ffi runtime lookup: {sys.platform}")


def runtime_library_path(mode: str = "release") -> Path:
    profile = build_profile(mode)
    if is_windows():
        return (
            ROOT / "target" / windows_target_triple() / profile / runtime_library_name()
        )
    return ROOT / "target" / profile / runtime_library_name()


def runtime_import_library_path(runtime: Path | None = None) -> Path:
    library = runtime or runtime_library_path()
    return Path(str(library) + ".lib")


def windows_lib_machine() -> str:
    target = windows_target_triple()
    if target.startswith("x86_64"):
        return "x64"
    if target.startswith("aarch64") or target.startswith("arm64"):
        return "arm64"
    if target.startswith("i686"):
        return "x86"
    raise SystemExit(f"unsupported Windows target for import library: {target}")


def coff_export_names(dll: Path) -> list[str]:
    readobj = shutil.which("llvm-readobj")
    if readobj is None:
        raise SystemExit(
            "llvm-readobj is required to generate the Windows runtime import library"
        )
    result = run([readobj, "--coff-exports", str(dll)], capture=True)
    names = re.findall(r"^\s*Name:\s+(\S+)\s*$", result.stdout, re.MULTILINE)
    if not names:
        raise SystemExit(f"could not find exports in {dll}")
    return names


def generate_windows_import_library(dll: Path, implib: Path) -> None:
    lib_tool = shutil.which("llvm-lib") or shutil.which("lib")
    if lib_tool is None:
        raise SystemExit(
            "llvm-lib or lib.exe is required to generate the Windows runtime "
            "import library"
        )
    exports = coff_export_names(dll)
    with tempfile.TemporaryDirectory(prefix="openpit-implib-") as temp_dir:
        definition = Path(temp_dir) / "openpit_ffi.def"
        definition.write_text(
            "\n".join(["LIBRARY openpit_ffi.dll", "EXPORTS", *exports]) + "\n",
            encoding="utf-8",
        )
        run(
            [
                lib_tool,
                f"/def:{definition}",
                f"/machine:{windows_lib_machine()}",
                f"/out:{implib}",
            ]
        )
    if not implib.is_file():
        raise SystemExit(f"failed to generate Windows runtime import library: {implib}")


def ensure_windows_runtime_import_library(runtime: Path | None = None) -> Path:
    library = runtime or runtime_library_path()
    implib = runtime_import_library_path(library)
    if not library.is_file():
        raise SystemExit(f"Windows runtime library not found at {library}")
    if implib.is_file() and implib.stat().st_mtime_ns >= library.stat().st_mtime_ns:
        return implib
    generate_windows_import_library(library, implib)
    return implib


def go_runtime_platform() -> tuple[str, str]:
    arch = host_arch()
    if sys.platform == "darwin":
        return f"darwin-{arch}", "libopenpit_ffi.dylib"
    if is_windows() and arch == "amd64":
        return "windows-amd64", "openpit_ffi.dll"
    if sys.platform.startswith("linux"):
        return f"linux-{arch}", "libopenpit_ffi.so"
    raise SystemExit(f"unsupported OS for go runtime embed: {sys.platform}")


def ctest_args(build_dir: str, mode: str) -> list[str]:
    args = ["ctest", "--test-dir", build_dir, "--output-on-failure"]
    if is_windows():
        args.extend(["--build-config", cmake_build_type(mode)])
    return args


def command_mkdir(args: argparse.Namespace) -> None:
    for path in args.paths:
        (ROOT / path).mkdir(parents=True, exist_ok=True)


def command_clean(_: argparse.Namespace) -> None:
    target = ROOT / "target"
    if not target.is_dir():
        return
    # Keep caches other just recipes place directly under target/ (see the
    # justfile exports for GOCACHE, PIP_CACHE_DIR, GOLANGCI_LINT_CACHE, and
    # node_dir) plus cargo-llvm-cov output and rust-analyzer's own build dir.
    preserved = {
        "go-cache",
        "pip-cache",
        "golangci-lint-cache",
        "node",
        "llvm-cov",
        "rust-analyzer",
    }
    removed = 0
    for entry in sorted(target.iterdir()):
        if entry.name in preserved:
            continue
        relative = entry.relative_to(ROOT)
        if entry.is_dir():
            shutil.rmtree(entry, onerror=remove_readonly)
        else:
            entry.unlink(missing_ok=True)
        print(f"removed {relative}")
        removed += 1
    noun = "entry" if removed == 1 else "entries"
    print(f"clean: removed {removed} {noun} under target")


def command_ensure_python_env(args: argparse.Namespace) -> None:
    venv_dir = Path(args.venv_dir)
    python_path = Path(args.python_path)
    if not python_path.is_file():
        run([sys.executable, "-m", "venv", str(venv_dir)])
    run([str(python_path), "-m", "pip", "install", "-r", args.requirements])


def command_cargo_doc_warnings(_: argparse.Namespace) -> None:
    env = os.environ.copy()
    env["RUSTDOCFLAGS"] = "-D warnings"
    run(
        [
            "cargo",
            "doc",
            "--workspace",
            "--exclude",
            "openpit-python",
            "--no-deps",
            "--all-features",
            "--locked",
            "-q",
        ],
        env=env,
    )


def command_test_rust(args: argparse.Namespace) -> None:
    release_args = cargo_release_args(args.mode)
    nextest_runs = [
        [
            "run",
            *release_args,
            "--workspace",
            "--exclude",
            "openpit-python",
            "--locked",
            "--status-level",
            "fail",
            "--final-status-level",
            "fail",
        ],
        [
            "run",
            *release_args,
            "-p",
            "openpit",
            "--all-features",
            "--locked",
            "--status-level",
            "fail",
            "--final-status-level",
            "fail",
        ],
    ]
    for nextest_args in nextest_runs:
        try:
            run(["cargo", "nextest", *nextest_args])
        except subprocess.CalledProcessError:
            if shutil.which("cargo-nextest") is None:
                print(
                    "\n  error: cargo-nextest is required to run Rust tests.\n",
                    file=sys.stderr,
                )
            raise

    run(["cargo", "test", *release_args, "--workspace", "--doc", "--locked"])
    run(
        [
            "cargo",
            "test",
            *release_args,
            "-p",
            "openpit",
            "--all-features",
            "--doc",
            "--locked",
        ]
    )


def command_test_python(_: argparse.Namespace) -> None:
    run(["just", "_pytest", "bindings/python/tests"])
    for example_dir in sorted((ROOT / "examples" / "python").iterdir()):
        if not example_dir.is_dir():
            continue
        if not (example_dir / "main.py").is_file():
            continue
        requirements = example_dir / "requirements.txt"
        if requirements.is_file():
            run([sys.executable, "-m", "pip", "install", "-r", str(requirements)])
        run(["just", "_pytest", str(example_dir.relative_to(ROOT))])


def command_run_python_spot_table(args: argparse.Namespace) -> None:
    command = [
        sys.executable,
        "examples/python/spot_table/main.py",
        "--table",
        str((ROOT / args.test_file).resolve()),
    ]
    if args.min_duration:
        command.extend(["--min-duration", args.min_duration])
    run(command)


def command_check_gofmt(args: argparse.Namespace) -> None:
    result = run(["gofmt", "-l", *args.paths], capture=True)
    unformatted = result.stdout.strip()
    if unformatted:
        print(unformatted)
        raise SystemExit(1)


def ci_version(name: str) -> str:
    versions = ROOT / ".github" / "ci-versions.env"
    if not versions.is_file():
        raise SystemExit(f"{versions} not found; expected CI version pins")
    for line in versions.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator and key == name:
            return value.strip()
    raise SystemExit(f"{name} not found in {versions}")


def golangci_lint_version(output: str) -> str | None:
    first_line = output.splitlines()[0] if output else ""
    match = re.search(r"\bversion\s+v?(\d+\.\d+\.\d+)\b", first_line)
    if match is None:
        return None
    return match.group(1)


def command_check_golangci_lint(_: argparse.Namespace) -> None:
    expected = ci_version("CI_GOLANGCI_LINT")
    executable = shutil.which("golangci-lint")
    if executable is None:
        raise SystemExit(
            f"golangci-lint v{expected} is required; it was not found on PATH"
        )
    with tempfile.TemporaryDirectory(prefix="openpit-golangci-version-") as temp_dir:
        result = run(["golangci-lint", "version"], cwd=Path(temp_dir), capture=True)
    first_line = result.stdout.splitlines()[0] if result.stdout else ""
    if golangci_lint_version(result.stdout) == expected:
        return
    raise SystemExit(
        f"golangci-lint v{expected} is required to match CI/CD; got: {first_line}"
    )


def command_go_cover_summary(_: argparse.Namespace) -> None:
    result = run(
        ["go", "tool", "cover", "-func=coverage.out"],
        cwd=ROOT / "bindings/go",
        capture=True,
    )
    for line in result.stdout.splitlines():
        if "100.0%" not in line:
            print(line)


def c_readme_example() -> str:
    readme = ROOT / "bindings" / "c" / "README.md"
    blocks: list[str] = []
    current: list[str] | None = None
    for line in readme.read_text(encoding="utf-8").splitlines():
        if line == "```c":
            current = []
            continue
        if line == "```" and current is not None:
            blocks.append("\n".join(current))
            current = None
            continue
        if current is not None:
            current.append(line)
    return "\n\n".join(blocks) + "\n"


def c_compiler_command() -> list[str]:
    cc = os.environ.get("CC")
    if cc:
        return shlex.split(cc)
    candidates = ["clang", "cc", "gcc"] if is_windows() else ["cc", "clang", "gcc"]
    for candidate in candidates:
        executable = shutil.which(candidate)
        if executable is not None:
            return [executable]
    expected = "clang" if is_windows() else "cc"
    raise SystemExit(
        f"missing executable: {expected}; install a C compiler or add it to PATH"
    )


def c_readme_syntax_command(source: Path) -> list[str]:
    compiler = c_compiler_command()
    name = Path(compiler[0]).name.lower()
    if name in {"cl", "cl.exe"}:
        return [
            *compiler,
            "/nologo",
            "/std:c11",
            "/Zs",
            "/I",
            "bindings/c",
            str(source),
        ]
    return [*compiler, "-std=c11", "-fsyntax-only", "-I", "bindings/c", str(source)]


def command_compile_c_readme_examples(_: argparse.Namespace) -> None:
    with tempfile.TemporaryDirectory(prefix="openpit-c-readme-") as temp_dir:
        source = Path(temp_dir) / "openpit_readme_example.c"
        source.write_text(c_readme_example(), encoding="utf-8")
        run(c_readme_syntax_command(source))


def command_build_cpp(args: argparse.Namespace) -> None:
    mode = build_mode(args.mode)
    env = os.environ.copy()
    configure_windows_temp_env(env)
    lib = runtime_library_path(mode)
    cmake_args = [
        f"-DOPENPIT_RUNTIME_LIBRARY={lib}",
        f"-DCMAKE_BUILD_TYPE={cmake_build_type(mode)}",
    ]
    if is_windows():
        import_library = ensure_windows_runtime_import_library(lib)
        cmake_args.append(f"-DOPENPIT_RUNTIME_IMPORT_LIBRARY={import_library}")

    source = "bindings/cpp" if args.kind == "binding" else "examples/cpp"
    build = cpp_build_dir(args.kind, mode)
    platform_args = cmake_platform_args()
    prepare_cmake_build_dir(build, platform_args)
    run(["cmake", "-S", source, "-B", build, *platform_args, *cmake_args], env=env)
    run(["cmake", "--build", build, "--config", cmake_build_type(mode)], env=env)


def command_ctest(args: argparse.Namespace) -> None:
    run(ctest_args(args.build_dir, args.mode))


def find_executable(base: Path, name: str) -> Path:
    suffix = ".exe" if is_windows() else ""
    wanted = name + suffix
    candidates = sorted(path for path in base.rglob(wanted) if path.is_file())
    if candidates:
        return candidates[0]
    raise SystemExit(f"could not find executable {wanted} under {base}")


def command_run_cpp(args: argparse.Namespace) -> None:
    exe = find_executable(
        ROOT / "examples" / "cpp" / f"build-{build_profile(args.mode)}" / args.name,
        args.name,
    )
    command = [str(exe)]
    if args.table:
        command.extend(["--table", str((ROOT / args.table).resolve())])
    run(command)


def cpp_sources(*roots: str, examples: bool = False) -> list[Path]:
    files: list[Path] = []
    for root in roots:
        for path in (ROOT / root).rglob("*"):
            if not path.is_file() or path.suffix not in {".hpp", ".cpp"}:
                continue
            parts = path.relative_to(ROOT).parts
            if examples and any(
                part == "build" or part.startswith("build-") for part in parts
            ):
                continue
            files.append(path)
    return sorted(files)


def command_cpp_format(args: argparse.Namespace) -> None:
    files = [
        *cpp_sources("bindings/cpp/include", "bindings/cpp/test", "e2e/clients/cpp"),
        *cpp_sources("examples/cpp", examples=True),
    ]
    if not files:
        return
    base_args = ["clang-format"]
    base_args.extend(["--dry-run", "-Werror"] if args.check else ["-i"])
    for group in chunks(files, 100):
        run([*base_args, *(str(path) for path in group)])


def command_lint_cpp(_: argparse.Namespace) -> None:
    env = os.environ.copy()
    configure_windows_temp_env(env)
    repo = str(ROOT).replace("\\", "/")
    binding_build = cpp_build_dir("binding", "debug")
    examples_build = cpp_build_dir("examples", "debug")
    tidy_args = ["--extra-arg=-std=gnu++17"]
    if sys.platform == "darwin":
        sdk = run(["xcrun", "--show-sdk-path"], capture=True).stdout.strip()
        tidy_args.extend(["--extra-arg=-isysroot", f"--extra-arg={sdk}"])

    platform_args = cmake_platform_args()
    prepare_cmake_build_dir(binding_build, platform_args)
    run(
        [
            "cmake",
            "-S",
            "bindings/cpp",
            "-B",
            binding_build,
            *platform_args,
            "-DCMAKE_BUILD_TYPE=Debug",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
        ],
        env=env,
    )
    if is_windows() and not (ROOT / binding_build / "compile_commands.json").is_file():
        print(
            "skipping clang-tidy; CMake did not create "
            f"{binding_build}/compile_commands.json",
            file=sys.stderr,
        )
        return
    header_filter = json.dumps([{"name": f"{repo}/bindings/cpp/include/.*"}])
    binding_tests = cpp_sources("bindings/cpp/test")
    for group in chunks(binding_tests, 50):
        command = [
            "clang-tidy",
            *tidy_args,
            "--config-file",
            "bindings/cpp/.clang-tidy",
            f"--line-filter={header_filter}",
        ]
        command.extend(["-p", binding_build])
        command.extend(str(path) for path in group)
        run(command, env=env)

    prepare_cmake_build_dir(examples_build, platform_args)
    run(
        [
            "cmake",
            "-S",
            "examples/cpp",
            "-B",
            examples_build,
            *platform_args,
            "-DCMAKE_BUILD_TYPE=Debug",
            "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
        ],
        env=env,
    )
    examples = [
        path
        for path in cpp_sources("examples/cpp", examples=True)
        if "test" not in path.relative_to(ROOT).parts
        and not path.name.endswith("_test.cpp")
    ]
    for group in chunks(examples, 50):
        command = [
            "clang-tidy",
            *tidy_args,
            "--config-file",
            "bindings/cpp/.clang-tidy",
        ]
        command.extend(["-p", examples_build])
        command.extend(str(path) for path in group)
        run(command, env=env)


def command_gen_docs_cpp(_: argparse.Namespace) -> None:
    missing_tools = [tool for tool in ("doxygen", "dot") if shutil.which(tool) is None]
    if missing_tools:
        if not (ROOT / "docs" / "cpp-api" / "index.html").is_file():
            tools = ", ".join(missing_tools)
            raise SystemExit(f"{tools} required to generate docs/cpp-api")
        tools = ", ".join(missing_tools)
        print(
            f"skipping docs/cpp-api generation; missing {tools}",
            file=sys.stderr,
        )
        run([sys.executable, "scripts/_generate_api_c_sitemap.py"])
        return
    shutil.rmtree(ROOT / "docs" / "cpp-api", ignore_errors=True)
    run(["doxygen", "bindings/cpp/Doxyfile"])
    normalize_doxygen_mainpage_anchor()
    run([sys.executable, "scripts/_generate_api_c_sitemap.py"])


def normalize_doxygen_mainpage_anchor() -> None:
    index = ROOT / "docs" / "cpp-api" / "index.html"
    if not index.is_file():
        return
    text = index.read_text(encoding="utf-8")
    normalized = re.sub(
        r'id="md_[^"]*DoxygenMainPage"',
        'id="openpit-cpp-sdk-mainpage"',
        text,
        count=1,
    )
    if normalized != text:
        index.write_text(normalized, encoding="utf-8", newline="\n")


def command_build_ffi(args: argparse.Namespace) -> None:
    mode = build_mode(args.mode)
    env = os.environ.copy()
    cargo = cargo_command()
    rustup_fallback = configure_windows_rust_env(env, cargo)
    command = [
        *cargo,
        "build",
        "-p",
        "openpit-ffi",
        *cargo_release_args(mode),
        "--locked",
        "-q",
    ]
    if is_windows():
        command_prefix = cargo[:-1] if cargo[-1] == "cargo" else []
        ensure_windows_target_installed(command_prefix)
        configure_windows_msvc_linker_env(env)
        rustflags = env.get("RUSTFLAGS", "")
        env["RUSTFLAGS"] = f"{rustflags} -C target-feature=+crt-static".strip()
        command.extend(["--target", windows_target_triple()])
    run(command, env=env)
    if rustup_fallback:
        source = (
            Path(env["CARGO_TARGET_DIR"])
            / windows_target_triple()
            / build_profile(mode)
            / runtime_library_name()
        )
        destination = runtime_library_path(mode)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    if is_windows():
        ensure_windows_runtime_import_library(runtime_library_path(mode))


def command_python_develop(args: argparse.Namespace) -> None:
    mode = build_mode(args.mode)
    env = os.environ.copy()
    command = [sys.executable, "-m", "maturin", "develop", "-q"]
    if mode == "release":
        command.append("--release")
    command.extend(["--manifest-path", "bindings/python/Cargo.toml"])
    if is_windows():
        cargo = cargo_command()
        configure_windows_rust_env(env, cargo)
        command_prefix = cargo[:-1] if cargo[-1] == "cargo" else []
        ensure_windows_target_installed(command_prefix)
        configure_windows_msvc_linker_env(env)
        command.extend(["--target", windows_target_triple()])
    run(command, env=env)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_with_sha256(source: Path, destination: Path) -> None:
    shutil.copy2(source, destination)
    destination.with_name(destination.name + ".sha256").write_text(
        f"{sha256_file(destination)}\n",
        encoding="utf-8",
    )


def command_package_runtime(args: argparse.Namespace) -> None:
    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    source = ROOT / "target" / args.target / "release" / args.lib_name
    destination = dist / f"openpit-ffi--{args.goos}-{args.goarch}-{args.lib_name}"
    copy_with_sha256(source, destination)
    import_library = Path(str(source) + ".lib")
    if import_library.is_file():
        copy_with_sha256(import_library, Path(str(destination) + ".lib"))
    shutil.copy2(ROOT / "bindings" / "c" / "openpit.h", dist / "openpit.h")
    shutil.copy2(ROOT / "LICENSE", dist / "LICENSE")
    shutil.copy2(ROOT / "OWNERS", dist / "OWNERS")


def command_go_embed_runtime(args: argparse.Namespace) -> None:
    mode = build_mode(args.mode)
    platform_name, lib_name = go_runtime_platform()
    destination = ROOT / "bindings" / "go" / "internal" / "runtime" / platform_name
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(runtime_library_path(mode), destination / lib_name)


def windows_cgo_compiler_command(compiler: Sequence[str]) -> str:
    compiler_key = tuple(compiler)
    cached = _WINDOWS_CGO_COMPILER_COMMANDS.get(compiler_key)
    if cached is not None:
        return cached
    wrapper_dir = Path(tempfile.mkdtemp(prefix="openpit-go-cgo-"))
    _WINDOWS_CGO_WRAPPER_DIRS.add(wrapper_dir)
    wrapper = wrapper_dir / "cc_wrapper.py"
    wrapper.write_text(
        "\n".join(
            [
                "from __future__ import annotations",
                "",
                "import subprocess",
                "import sys",
                "",
                'filtered = {"-mthreads", "-s"}',
                "args = [arg for arg in sys.argv[1:] if arg not in filtered]",
                "raise SystemExit(subprocess.call(args))",
                "",
            ]
        ),
        encoding="utf-8",
    )
    command = subprocess.list2cmdline([sys.executable, str(wrapper), *compiler])
    _WINDOWS_CGO_COMPILER_COMMANDS[compiler_key] = command
    return command


def go_env(mode: str) -> dict[str, str]:
    env = os.environ.copy()
    env["CGO_ENABLED"] = "1"
    env["OPENPIT_RUNTIME_LIBRARY_PATH"] = str(runtime_library_path(mode))
    if is_windows():
        if "CC" not in env:
            env["CC"] = os.environ.get(
                "PIT_WINDOWS_CGO_CC"
            ) or windows_cgo_compiler_command(["clang", "-fuse-ld=lld"])
        if "CXX" not in env:
            env["CXX"] = os.environ.get(
                "PIT_WINDOWS_CGO_CXX"
            ) or windows_cgo_compiler_command(["clang++", "-fuse-ld=lld"])
    return env


def command_go_examples(args: argparse.Namespace) -> None:
    env = go_env(args.mode)
    command = shell_command_from_parts(args.command)
    for example_dir in sorted((ROOT / "examples" / "go").iterdir()):
        if not example_dir.is_dir() or not (example_dir / "go.mod").is_file():
            continue
        print(f">> {example_dir.relative_to(ROOT)}", file=sys.stderr)
        run_shell(command, cwd=example_dir, env=env)


def command_go_in(args: argparse.Namespace) -> None:
    run_shell(
        shell_command_from_parts(args.command),
        cwd=ROOT / args.directory,
        env=go_env(args.mode),
    )


def shell_command_from_parts(parts: Sequence[str]) -> list[str]:
    return [str(clean_cli_value(part)) for part in parts]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparser = subparsers.add_parser("mkdir")
    subparser.add_argument("paths", nargs="+")
    subparser.set_defaults(func=command_mkdir)

    subparsers.add_parser("clean").set_defaults(func=command_clean)

    subparser = subparsers.add_parser("ensure-python-env")
    subparser.add_argument("venv_dir")
    subparser.add_argument("python_path")
    subparser.add_argument("requirements")
    subparser.set_defaults(func=command_ensure_python_env)

    subparser = subparsers.add_parser("ensure-node")
    subparser.add_argument("node_dir")
    subparser.set_defaults(func=command_ensure_node)

    subparsers.add_parser("cargo-doc-warnings").set_defaults(
        func=command_cargo_doc_warnings
    )
    subparser = subparsers.add_parser("test-rust")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_test_rust)
    subparsers.add_parser("test-python").set_defaults(func=command_test_python)

    subparser = subparsers.add_parser("python-develop")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_python_develop)

    subparser = subparsers.add_parser("run-python-spot-table")
    subparser.add_argument("test_file")
    subparser.add_argument("--min-duration")
    subparser.set_defaults(func=command_run_python_spot_table)

    subparser = subparsers.add_parser("check-gofmt")
    subparser.add_argument("paths", nargs="+")
    subparser.set_defaults(func=command_check_gofmt)

    subparsers.add_parser("check-golangci-lint").set_defaults(
        func=command_check_golangci_lint
    )

    subparsers.add_parser("go-cover-summary").set_defaults(
        func=command_go_cover_summary
    )
    subparsers.add_parser("compile-c-readme-examples").set_defaults(
        func=command_compile_c_readme_examples
    )

    subparser = subparsers.add_parser("build-cpp")
    subparser.add_argument("kind", choices=["binding", "examples"])
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_build_cpp)

    subparser = subparsers.add_parser("ctest")
    subparser.add_argument("build_dir")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_ctest)

    subparser = subparsers.add_parser("run-cpp")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.add_argument("name")
    subparser.add_argument("--table")
    subparser.set_defaults(func=command_run_cpp)

    subparser = subparsers.add_parser("cpp-format")
    subparser.add_argument("--check", action="store_true")
    subparser.set_defaults(func=command_cpp_format)

    subparsers.add_parser("lint-cpp").set_defaults(func=command_lint_cpp)
    subparsers.add_parser("gen-docs-cpp").set_defaults(func=command_gen_docs_cpp)
    subparser = subparsers.add_parser("build-ffi")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_build_ffi)

    subparser = subparsers.add_parser("package-runtime")
    subparser.add_argument("target")
    subparser.add_argument("goos")
    subparser.add_argument("goarch")
    subparser.add_argument("lib_name")
    subparser.set_defaults(func=command_package_runtime)

    subparser = subparsers.add_parser("go-embed-runtime")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.set_defaults(func=command_go_embed_runtime)

    subparser = subparsers.add_parser("go-examples")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.add_argument("command", nargs=argparse.REMAINDER)
    subparser.set_defaults(func=command_go_examples)

    subparser = subparsers.add_parser("go-in")
    subparser.add_argument("mode", choices=sorted(BUILD_MODES))
    subparser.add_argument("directory")
    subparser.add_argument("command", nargs=argparse.REMAINDER)
    subparser.set_defaults(func=command_go_in)

    return parser


def clean_cli_value(value: object) -> object:
    if (
        isinstance(value, str)
        and len(value) >= 2
        and value[0] == value[-1]
        and value[0] in {'"', "'"}
    ):
        return value[1:-1]
    if isinstance(value, list):
        return [clean_cli_value(item) for item in value]
    return value


def clean_cli_args(args: argparse.Namespace) -> argparse.Namespace:
    for name, value in vars(args).items():
        setattr(args, name, clean_cli_value(value))
    return args


def main() -> None:
    args = clean_cli_args(build_parser().parse_args())
    try:
        args.func(args)
    except subprocess.CalledProcessError as exc:
        raise SystemExit(exc.returncode) from None


if __name__ == "__main__":
    main()
