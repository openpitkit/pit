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

from __future__ import annotations

import importlib.util
import io
import json
import os
import sys
import time
from pathlib import Path

import pytest

SCRIPT_PATH = Path(__file__).resolve().parents[1] / "just_helpers.py"


def load_module():
    spec = importlib.util.spec_from_file_location("just_helpers", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_clean_keeps_project_local_tools(tmp_path, monkeypatch) -> None:
    module = load_module()
    target = tmp_path / "target"
    for name in ("node", "cargo-about", "debug"):
        (target / name).mkdir(parents=True)
    monkeypatch.setattr(module, "ROOT", tmp_path)

    module.command_clean(module.argparse.Namespace())

    assert sorted(entry.name for entry in target.iterdir()) == ["cargo-about", "node"]


def test_c_readme_syntax_command_prefers_windows_clang(monkeypatch) -> None:
    module = load_module()

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.delenv("CC", raising=False)
    monkeypatch.setattr(
        module.shutil,
        "which",
        lambda name: r"C:\LLVM\bin\clang.exe" if name == "clang" else None,
    )

    command = module.c_readme_syntax_command(Path("example.c"))

    assert command == [
        r"C:\LLVM\bin\clang.exe",
        "-std=c11",
        "-fsyntax-only",
        "-I",
        "bindings/c",
        "example.c",
    ]


def test_c_compiler_command_uses_cc_environment(monkeypatch) -> None:
    module = load_module()

    monkeypatch.setenv("CC", "custom-cc -Wall")

    assert module.c_compiler_command() == ["custom-cc", "-Wall"]


def test_shell_command_from_parts_preserves_argument_boundaries(monkeypatch) -> None:
    module = load_module()
    command = [
        "go",
        "test",
        "path with spaces",
        'embedded "quote"',
        "semi;colon",
        "pipe|value",
        "dollar$PATH",
    ]

    for windows in (False, True):
        monkeypatch.setattr(module, "is_windows", lambda windows=windows: windows)

        assert module.shell_command_from_parts(command) == command


def test_shell_command_from_parts_dequotes_cli_parts() -> None:
    module = load_module()

    command = module.shell_command_from_parts(
        ['"go"', "'path with spaces'", 'embedded "quote"', "semi;colon"]
    )

    assert command == ["go", "path with spaces", 'embedded "quote"', "semi;colon"]


def test_clean_cli_value_dequotes_matching_outer_quotes() -> None:
    module = load_module()

    assert module.clean_cli_value('"path with spaces"') == "path with spaces"
    assert module.clean_cli_value("'embedded \"quote\"'") == 'embedded "quote"'
    assert module.clean_cli_value("\"mismatched'") == "\"mismatched'"
    assert module.clean_cli_value(["'go'", '"test path"', "plain"]) == [
        "go",
        "test path",
        "plain",
    ]


def test_clean_cli_args_dequotes_namespace_values() -> None:
    module = load_module()
    args = module.argparse.Namespace(
        directory='"bindings/go"',
        command=["'go'", '"test path"', "plain"],
    )

    cleaned = module.clean_cli_args(args)

    assert cleaned is args
    assert cleaned.directory == "bindings/go"
    assert cleaned.command == ["go", "test path", "plain"]


def test_run_passes_argv_without_shell(tmp_path, monkeypatch) -> None:
    module = load_module()
    observed = {}
    command = ["tool", "path with spaces", "semi;colon", "pipe|value"]

    def fake_subprocess_run(args, **kwargs):
        observed["args"] = args
        observed["kwargs"] = kwargs
        return module.subprocess.CompletedProcess(args, 0)

    monkeypatch.setattr(module.subprocess, "run", fake_subprocess_run)

    module.run(command, cwd=tmp_path, env={"OPENPIT_TEST": "1"})

    assert observed["args"] == command
    assert "shell" not in observed["kwargs"]
    assert observed["kwargs"]["cwd"] == tmp_path
    assert observed["kwargs"]["env"] == {"OPENPIT_TEST": "1"}
    assert observed["kwargs"]["check"] is True


def test_run_missing_executable_raises_system_exit(tmp_path) -> None:
    module = load_module()
    missing = "__openpit_missing_executable_for_just_helper_tests__"

    with pytest.raises(SystemExit, match=missing):
        module.run([missing], cwd=tmp_path)


def test_run_shell_missing_executable_raises_system_exit(tmp_path) -> None:
    module = load_module()
    missing = "__openpit_missing_shell_executable_for_just_helper_tests__"

    with pytest.raises(SystemExit, match=missing):
        module.run_shell([missing], cwd=tmp_path, env={})


def test_run_shell_delegates_argv(tmp_path, monkeypatch) -> None:
    module = load_module()
    observed = {}
    command = ["go", "test", "path with spaces", "semi;colon"]

    def fake_run(args, **kwargs):
        observed["args"] = args
        observed["kwargs"] = kwargs

    monkeypatch.setattr(module, "run", fake_run)

    module.run_shell(command, cwd=tmp_path, env={"OPENPIT_TEST": "1"})

    assert observed["args"] == command
    assert observed["kwargs"] == {"cwd": tmp_path, "env": {"OPENPIT_TEST": "1"}}


@pytest.mark.parametrize(
    ("system", "machine", "expected"),
    [
        ("Darwin", "arm64", "node-v22.12.0-darwin-arm64.tar.xz"),
        ("Linux", "x86_64", "node-v22.12.0-linux-x64.tar.xz"),
        ("Windows", "AMD64", "node-v22.12.0-win-x64.zip"),
    ],
)
def test_node_archive_name_uses_supported_platforms(
    monkeypatch, system: str, machine: str, expected: str
) -> None:
    module = load_module()
    monkeypatch.setattr(module.platform, "system", lambda: system)
    monkeypatch.setattr(module.platform, "machine", lambda: machine)

    assert module.node_archive_name("22.12.0") == expected


def test_node_archive_name_rejects_unpinned_version() -> None:
    module = load_module()

    with pytest.raises(SystemExit, match="invalid CI_NODE"):
        module.node_archive_name("22")


def test_ensure_node_skips_download_when_runtime_matches_ci(
    tmp_path, monkeypatch
) -> None:
    module = load_module()
    monkeypatch.setattr(module, "ci_version", lambda name: "22.12.0")
    monkeypatch.setattr(module, "node_runtime_version", lambda path: "22.12.0")
    monkeypatch.setattr(
        module,
        "install_node_runtime",
        lambda path, version: pytest.fail("install should not be called"),
    )

    module.command_ensure_node(module.argparse.Namespace(node_dir=str(tmp_path)))


def test_ensure_node_replaces_outdated_runtime(tmp_path, monkeypatch) -> None:
    module = load_module()
    observed = {}
    monkeypatch.setattr(module, "ci_version", lambda name: "22.12.0")
    monkeypatch.setattr(module, "node_runtime_version", lambda path: "26.4.0")

    def install(path, version) -> None:
        observed["path"] = path
        observed["version"] = version

    monkeypatch.setattr(module, "install_node_runtime", install)

    module.command_ensure_node(module.argparse.Namespace(node_dir=str(tmp_path)))

    assert observed == {"path": tmp_path, "version": "22.12.0"}


def test_golangci_lint_version_accepts_plain_version() -> None:
    module = load_module()

    version = module.golangci_lint_version(
        "golangci-lint has version 2.12.2 built with go1.26.2 "
        "from c0d3ddc9 on 2026-05-06T11:07:58Z\n"
    )

    assert version == "2.12.2"


def test_golangci_lint_version_accepts_prefixed_version() -> None:
    module = load_module()

    version = module.golangci_lint_version(
        "golangci-lint has version v2.12.2 built with go1.26.2\n"
    )

    assert version == "2.12.2"


def test_golangci_lint_version_rejects_unparseable_output() -> None:
    module = load_module()

    assert module.golangci_lint_version("golangci-lint dev build\n") is None


def test_golangci_lint_version_rejects_empty_output() -> None:
    module = load_module()

    assert module.golangci_lint_version("") is None


@pytest.mark.parametrize("suffix", ["-rc.1", "-dev", "+custom"])
def test_golangci_lint_version_rejects_nonrelease_build(suffix) -> None:
    module = load_module()

    assert (
        module.golangci_lint_version(f"golangci-lint has version 2.12.2{suffix}")
        is None
    )


@pytest.fixture
def golangci_lint_check(tmp_path, monkeypatch):
    module = load_module()
    monkeypatch.setattr(module, "ROOT", tmp_path)
    pins = tmp_path / ".github" / "ci-versions.env"
    pins.parent.mkdir()
    pins.write_text("CI_GOLANGCI_LINT=2.12.2\n", encoding="utf-8")
    monkeypatch.setattr(module.shutil, "which", lambda name: "/tools/golangci-lint")

    def version_command(command, *, cwd, capture):
        assert command == ["/tools/golangci-lint", "version"]
        assert cwd != tmp_path
        assert cwd.is_dir()
        assert capture
        return module.subprocess.CompletedProcess(
            command, 0, stdout="golangci-lint has version 2.12.2 built with go1.26.2\n"
        )

    monkeypatch.setattr(module, "run", version_command)
    monkeypatch.setattr(
        module.urllib.request,
        "urlopen",
        lambda *args, **kwargs: pytest.fail(
            "offline version check must not use the network"
        ),
    )
    return module


def test_check_golangci_lint_matches_ci_without_network(golangci_lint_check, capsys):
    module = golangci_lint_check

    module.command_check_golangci_lint(
        module.build_parser().parse_args(["check-golangci-lint"])
    )

    output = capsys.readouterr().out
    assert "pinned: v2.12.2 (.github/ci-versions.env)" in output
    assert "matches CI/CD (/tools/golangci-lint)" in output


@pytest.mark.parametrize(
    "output",
    [
        "golangci-lint has version 2.11.0",
        "golangci-lint has version 2.15.0",
        "golangci-lint has version 2.12.2-rc.1",
        "golangci-lint dev build",
        "",
    ],
)
def test_check_golangci_lint_rejects_drift(golangci_lint_check, monkeypatch, output):
    module = golangci_lint_check
    monkeypatch.setattr(
        module,
        "run",
        lambda *args, **kwargs: module.subprocess.CompletedProcess(
            [], 0, stdout=output
        ),
    )

    with pytest.raises(SystemExit, match="version mismatch") as error:
        module.command_check_golangci_lint(
            module.build_parser().parse_args(["check-golangci-lint"])
        )

    assert "v2.12.2 is required to match CI/CD" in str(error.value)
    assert repr(output) in str(error.value)


def test_check_golangci_lint_rejects_missing_tool(golangci_lint_check, monkeypatch):
    module = golangci_lint_check
    monkeypatch.setattr(module.shutil, "which", lambda name: None)

    with pytest.raises(
        SystemExit, match="v2.12.2 is required; it was not found on PATH"
    ):
        module.command_check_golangci_lint(
            module.build_parser().parse_args(["check-golangci-lint"])
        )


@pytest.mark.parametrize("pin", ["", "latest", "2.12", "2.12.2-rc.1"])
def test_check_golangci_lint_rejects_invalid_pin(golangci_lint_check, monkeypatch, pin):
    module = golangci_lint_check
    monkeypatch.setattr(module, "ci_version", lambda name: pin)

    with pytest.raises(SystemExit, match="invalid CI_GOLANGCI_LINT version pin"):
        module.command_check_golangci_lint(
            module.build_parser().parse_args(["check-golangci-lint"])
        )


@pytest.mark.parametrize(
    ("latest", "message"),
    [
        ("2.15.0", "UPDATE AVAILABLE: pinned v2.12.2, latest stable v2.15.0"),
        ("2.12.2", "latest stable: v2.12.2; pin is current"),
        ("2.9.0", "pinned v2.12.2 is newer than latest stable v2.9.0"),
    ],
)
def test_check_golangci_lint_reports_updates_without_changing_pin(
    golangci_lint_check, monkeypatch, capsys, latest, message
):
    module = golangci_lint_check
    monkeypatch.setattr(module, "latest_golangci_lint_version", lambda: latest)

    module.command_check_golangci_lint(
        module.build_parser().parse_args(["check-golangci-lint", "--check-updates"])
    )

    assert message in capsys.readouterr().out
    assert module.ci_version("CI_GOLANGCI_LINT") == "2.12.2"


def test_latest_golangci_lint_version_reads_stable_release(monkeypatch):
    module = load_module()

    def latest_release(request, *, timeout):
        assert request.full_url == (
            "https://api.github.com/repos/golangci/golangci-lint/releases/latest"
        )
        assert request.get_header("Accept") == "application/vnd.github+json"
        assert timeout == 10
        return io.BytesIO(b'{"tag_name":"v2.15.0","draft":false,"prerelease":false}')

    monkeypatch.setattr(module.urllib.request, "urlopen", latest_release)

    assert module.latest_golangci_lint_version() == "2.15.0"


def test_check_golangci_lint_reports_update_and_drift_together(
    golangci_lint_check, monkeypatch, capsys
):
    module = golangci_lint_check
    monkeypatch.setattr(module, "latest_golangci_lint_version", lambda: "2.15.0")
    monkeypatch.setattr(
        module,
        "run",
        lambda *args, **kwargs: module.subprocess.CompletedProcess(
            [], 0, stdout="golangci-lint has version 2.15.0"
        ),
    )

    with pytest.raises(SystemExit, match="version mismatch"):
        module.command_check_golangci_lint(
            module.build_parser().parse_args(["check-golangci-lint", "--check-updates"])
        )

    assert (
        "UPDATE AVAILABLE: pinned v2.12.2, latest stable v2.15.0"
        in capsys.readouterr().out
    )


@pytest.mark.parametrize(
    "release",
    [
        [],
        {},
        {"tag_name": 215},
        {"tag_name": "v2.15.0-rc.1", "draft": False, "prerelease": True},
        {"tag_name": "v2.15.0", "draft": True, "prerelease": False},
        {"tag_name": "v2.15.0", "draft": False, "prerelease": True},
    ],
)
def test_latest_golangci_lint_version_rejects_invalid_release(monkeypatch, release):
    module = load_module()
    monkeypatch.setattr(
        module.urllib.request,
        "urlopen",
        lambda *args, **kwargs: io.BytesIO(json.dumps(release).encode()),
    )

    with pytest.raises(SystemExit, match="invalid .*golangci-lint release"):
        module.latest_golangci_lint_version()


@pytest.mark.parametrize("payload", [b"not JSON", b"\xff"])
def test_latest_golangci_lint_version_reports_bad_response(monkeypatch, payload):
    module = load_module()
    monkeypatch.setattr(
        module.urllib.request, "urlopen", lambda *args, **kwargs: io.BytesIO(payload)
    )

    with pytest.raises(SystemExit, match="could not check golangci-lint updates"):
        module.latest_golangci_lint_version()


@pytest.mark.parametrize("failure", ["http", "timeout"])
def test_latest_golangci_lint_version_reports_network_failure(monkeypatch, failure):
    module = load_module()

    def unavailable(*args, **kwargs):
        if failure == "timeout":
            raise TimeoutError("request timed out")
        raise module.urllib.error.HTTPError(
            "https://api.github.com", 403, "rate limit", {}, None
        )

    monkeypatch.setattr(module.urllib.request, "urlopen", unavailable)

    with pytest.raises(SystemExit, match="could not check golangci-lint updates"):
        module.latest_golangci_lint_version()


def test_lint_go_checks_version_on_unix_and_windows():
    recipes = (SCRIPT_PATH.parents[1] / "justfile").read_text(encoding="utf-8")
    lint_go = [line for line in recipes.splitlines() if line.startswith("lint-go:")]

    assert len(lint_go) == 2
    assert all(line.split()[1] == "check-golangci-lint" for line in lint_go)


def test_ci_version_missing_file_raises_clear_error(tmp_path, monkeypatch) -> None:
    module = load_module()
    monkeypatch.setattr(module, "ROOT", tmp_path)

    with pytest.raises(SystemExit, match="ci-versions.env not found"):
        module.ci_version("CI_GOLANGCI_LINT")


def test_windows_cgo_compiler_command_filters_mthreads(tmp_path, monkeypatch) -> None:
    module = load_module()
    created_dirs = []

    def fake_mkdtemp(prefix: str) -> str:
        wrapper_dir = tmp_path / f"{prefix}{len(created_dirs)}"
        wrapper_dir.mkdir()
        created_dirs.append(wrapper_dir)
        return str(wrapper_dir)

    monkeypatch.setattr(module.tempfile, "mkdtemp", fake_mkdtemp)

    command = module.windows_cgo_compiler_command(["clang", "-fuse-ld=lld"])

    wrapper = created_dirs[0] / "cc_wrapper.py"
    assert wrapper.is_file()
    assert str(wrapper) in command
    assert "clang" in command
    assert "-fuse-ld=lld" in command

    assert module.windows_cgo_compiler_command(["clang", "-fuse-ld=lld"]) == command
    assert len(created_dirs) == 1

    result = module.subprocess.run(
        [
            sys.executable,
            str(wrapper),
            sys.executable,
            "-c",
            "import sys; print(' '.join(sys.argv[1:]))",
            "-mthreads",
            "-s",
            "kept",
        ],
        check=True,
        stdout=module.subprocess.PIPE,
        text=True,
    )
    assert result.stdout.strip() == "kept"

    module._cleanup_windows_cgo_wrapper_dirs()
    assert not created_dirs[0].exists()


def test_go_env_uses_wrapped_windows_cgo_default(tmp_path, monkeypatch) -> None:
    module = load_module()
    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(
        module, "runtime_library_path", lambda mode: Path("runtime.dll")
    )
    created_dirs = []

    def fake_mkdtemp(prefix: str) -> str:
        wrapper_dir = tmp_path / f"{prefix}{len(created_dirs)}"
        wrapper_dir.mkdir()
        created_dirs.append(wrapper_dir)
        return str(wrapper_dir)

    monkeypatch.setattr(module.tempfile, "mkdtemp", fake_mkdtemp)
    monkeypatch.delenv("CC", raising=False)
    monkeypatch.delenv("CXX", raising=False)
    monkeypatch.delenv("PIT_WINDOWS_CGO_CC", raising=False)
    monkeypatch.delenv("PIT_WINDOWS_CGO_CXX", raising=False)

    env = module.go_env("debug")

    assert env["CGO_ENABLED"] == "1"
    assert env["OPENPIT_RUNTIME_LIBRARY_PATH"] == "runtime.dll"
    assert str(created_dirs[0] / "cc_wrapper.py") in env["CC"]
    assert str(created_dirs[1] / "cc_wrapper.py") in env["CXX"]
    assert "clang" in env["CC"]
    assert "clang++" in env["CXX"]

    second_env = module.go_env("debug")

    assert second_env["CC"] == env["CC"]
    assert second_env["CXX"] == env["CXX"]
    assert len(created_dirs) == 2


def test_go_env_uses_existing_compiler_environment_without_wrappers(
    monkeypatch,
) -> None:
    module = load_module()
    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(
        module, "runtime_library_path", lambda mode: Path("runtime.dll")
    )
    monkeypatch.setenv("CC", "custom-cc")
    monkeypatch.setenv("CXX", "custom-cxx")
    monkeypatch.setattr(
        module.tempfile,
        "mkdtemp",
        lambda **kwargs: pytest.fail("mkdtemp should not be called"),
    )

    env = module.go_env("debug")

    assert env["CC"] == "custom-cc"
    assert env["CXX"] == "custom-cxx"


def test_cargo_command_uses_plain_cargo_when_windows_host_matches(monkeypatch) -> None:
    module = load_module()
    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(
        module, "windows_target_triple", lambda: "x86_64-pc-windows-msvc"
    )
    monkeypatch.setattr(
        module, "rust_host_triple", lambda prefix=(): "x86_64-pc-windows-msvc"
    )

    assert module.cargo_command() == ["cargo"]


def test_cargo_command_uses_rustup_when_path_rustc_is_wrong_host(monkeypatch) -> None:
    module = load_module()
    target = "x86_64-pc-windows-msvc"

    def rust_host(prefix=()):
        if prefix == ["rustup", "run", f"stable-{target}"]:
            return target
        return "i686-pc-windows-msvc"

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(module, "windows_target_triple", lambda: target)
    monkeypatch.setattr(module, "rust_host_triple", rust_host)
    monkeypatch.setattr(module, "rustup_toolchain_installed", lambda toolchain: True)

    assert module.cargo_command() == ["rustup", "run", f"stable-{target}", "cargo"]


def test_configure_windows_rust_env_sets_rustup_proxy_path(
    tmp_path, monkeypatch
) -> None:
    module = load_module()
    target = "x86_64-pc-windows-msvc"
    rustup = tmp_path / "rustup.exe"
    rustup.write_text("", encoding="utf-8")
    env = {"Path": "existing"}

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(module, "windows_target_triple", lambda: target)
    monkeypatch.setattr(module.shutil, "which", lambda name: str(rustup))

    configured = module.configure_windows_rust_env(
        env, ["rustup", "run", f"stable-{target}", "cargo"]
    )

    assert configured is True
    assert env["Path"].startswith(str(tmp_path))
    assert env["RUSTUP_TOOLCHAIN"] == f"stable-{target}"
    assert Path(env["CARGO_TARGET_DIR"]).parts[-2:] == ("target", f"rustup-{target}")


def test_configure_windows_temp_env_uses_workspace_tmp(tmp_path, monkeypatch) -> None:
    module = load_module()
    env = {}
    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(module, "ROOT", tmp_path)

    module.configure_windows_temp_env(env)

    assert env["TEMP"] == str(tmp_path / ".tmp" / "subprocess-temp")
    assert env["TMP"] == env["TEMP"]
    assert Path(env["TEMP"]).is_dir()


def test_configure_windows_msvc_linker_env_uses_vc_tools_dir(
    tmp_path, monkeypatch
) -> None:
    module = load_module()
    linker = tmp_path / "VC" / "bin" / "Hostx64" / "x64" / "link.exe"
    linker.parent.mkdir(parents=True)
    linker.write_text("", encoding="utf-8")
    env = {"vctoolsinstalldir": str(tmp_path / "VC")}

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(module, "host_arch", lambda: "amd64")
    monkeypatch.setattr(
        module, "windows_target_triple", lambda: "x86_64-pc-windows-msvc"
    )

    module.configure_windows_msvc_linker_env(env)

    linker_var = "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"
    assert env[linker_var] == str(linker)


def test_configure_windows_msvc_linker_env_preserves_existing(monkeypatch) -> None:
    module = load_module()
    linker_var = "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"
    env = {linker_var: r"C:\Custom\link.exe"}

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(
        module, "windows_target_triple", lambda: "x86_64-pc-windows-msvc"
    )

    module.configure_windows_msvc_linker_env(env)

    assert env[linker_var] == r"C:\Custom\link.exe"


def test_configure_windows_msvc_linker_env_rejects_git_linker(monkeypatch) -> None:
    module = load_module()

    monkeypatch.setattr(module, "is_windows", lambda: True)
    monkeypatch.setattr(
        module, "windows_target_triple", lambda: "x86_64-pc-windows-msvc"
    )
    monkeypatch.setattr(module, "msvc_linker_candidates", lambda env, target: [])
    monkeypatch.setattr(
        module.shutil,
        "which",
        lambda name, **_: (
            r"C:\Program Files\Git\usr\bin\link.exe" if name == "link" else None
        ),
    )

    with pytest.raises(SystemExit, match="Git for Windows link.exe"):
        module.configure_windows_msvc_linker_env({})


def test_ensure_windows_runtime_import_library_generates_missing_implib(
    tmp_path, monkeypatch
) -> None:
    module = load_module()
    dll = tmp_path / "openpit_ffi.dll"
    dll.write_bytes(b"dll")
    readobj = tmp_path / "llvm-readobj.exe"
    lib_tool = tmp_path / "llvm-lib.exe"
    readobj.write_text("", encoding="utf-8")
    lib_tool.write_text("", encoding="utf-8")
    observed_definition = ""

    monkeypatch.setattr(module, "runtime_library_path", lambda: dll)
    monkeypatch.setattr(
        module, "windows_target_triple", lambda: "x86_64-pc-windows-msvc"
    )

    def which(name: str) -> str | None:
        if name == "llvm-readobj":
            return str(readobj)
        if name == "llvm-lib":
            return str(lib_tool)
        return None

    def fake_run(args, **kwargs):
        nonlocal observed_definition
        if args[0] == str(readobj):
            return module.subprocess.CompletedProcess(
                args,
                0,
                stdout=(
                    "Export {\n"
                    "  Name: openpit_create_engine_builder\n"
                    "}\n"
                    "Export {\n"
                    "  Name: openpit_destroy_engine\n"
                    "}\n"
                ),
            )
        if args[0] == str(lib_tool):
            definition = next(
                arg.removeprefix("/def:") for arg in args if arg.startswith("/def:")
            )
            output = next(
                arg.removeprefix("/out:") for arg in args if arg.startswith("/out:")
            )
            observed_definition = Path(definition).read_text(encoding="utf-8")
            Path(output).write_bytes(b"lib")
            return module.subprocess.CompletedProcess(args, 0)
        raise AssertionError(f"unexpected command: {args}")

    monkeypatch.setattr(module.shutil, "which", which)
    monkeypatch.setattr(module, "run", fake_run)

    implib = module.ensure_windows_runtime_import_library()

    assert implib == Path(str(dll) + ".lib")
    assert implib.is_file()
    assert "LIBRARY openpit_ffi.dll" in observed_definition
    assert "openpit_create_engine_builder" in observed_definition
    assert "openpit_destroy_engine" in observed_definition


def test_ensure_windows_runtime_import_library_regenerates_stale_implib(
    tmp_path, monkeypatch
) -> None:
    module = load_module()
    dll = tmp_path / "openpit_ffi.dll"
    implib = tmp_path / "openpit_ffi.dll.lib"
    dll.write_bytes(b"dll")
    implib.write_bytes(b"old")
    now_ns = time.time_ns()
    os.utime(implib, ns=(now_ns - 2_000_000_000, now_ns - 2_000_000_000))
    os.utime(dll, ns=(now_ns, now_ns))
    calls = 0

    monkeypatch.setattr(module, "runtime_library_path", lambda: dll)

    def fake_generate(library, output):
        nonlocal calls
        calls += 1
        assert library == dll
        assert output == implib
        output.write_bytes(b"new")

    monkeypatch.setattr(module, "generate_windows_import_library", fake_generate)

    assert module.ensure_windows_runtime_import_library() == implib
    assert calls == 1
