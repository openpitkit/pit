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
import sys
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
