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

import os
import shutil
import subprocess
import textwrap
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
VERSION = "1.2.3"
NOTES = "RELEASE_NOTES.md"
MARKER = f"<!-- release-version: {VERSION} -->\n"
BODY = "Adds account controls for grouped portfolios.\n"


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", str(repo), "-c", "core.hooksPath=/dev/null", *args],
        check=True,
        capture_output=True,
        text=True,
    )


@pytest.fixture
def repository(tmp_path):
    git(tmp_path, "init")
    git(tmp_path, "config", "user.name", "Release Test")
    git(tmp_path, "config", "user.email", "release@example.invalid")
    git(tmp_path, "config", "commit.gpgsign", "false")
    git(tmp_path, "commit", "--allow-empty", "-m", "Initial commit")
    return tmp_path


def commit_notes(repo, body=BODY, marker=MARKER):
    destination = repo / NOTES
    destination.write_text(marker + body, encoding="utf-8")
    git(repo, "add", NOTES)
    git(repo, "commit", "-m", "Add release notes")


def check_notes(repo, version=VERSION):
    return subprocess.run(
        [
            "just",
            "--justfile",
            str(REPO / "pipeline.just"),
            "--working-directory",
            str(repo),
            "check-release-notes",
            version,
        ],
        capture_output=True,
        text=True,
    )


def test_committed_notes_are_accepted(repository):
    commit_notes(repository)
    assert check_notes(repository).returncode == 0


@pytest.mark.parametrize("body", ["", " \n\t\r\n"])
def test_empty_notes_are_rejected(repository, body):
    commit_notes(repository, body)
    result = check_notes(repository)
    assert result.returncode != 0
    assert "not just whitespace" in result.stderr


@pytest.mark.parametrize("state", ["missing", "untracked", "staged"])
def test_notes_must_be_committed_for_the_exact_version(repository, state):
    if state != "missing":
        destination = repository / NOTES
        destination.write_text(MARKER + BODY, encoding="utf-8")
        if state == "staged":
            git(repository, "add", NOTES)
    result = check_notes(repository)
    assert result.returncode != 0
    assert f"Commit {NOTES}" in result.stderr


@pytest.mark.parametrize(
    "marker", ["", "<!-- release-version: 1.2.2 -->\n", "# 1.2.3\n"]
)
def test_notes_version_marker_must_match_target(repository, marker):
    commit_notes(repository, marker=marker)
    result = check_notes(repository)
    assert result.returncode != 0
    assert f"must start with {MARKER.strip()}" in result.stderr


def test_same_file_can_be_overwritten_for_the_next_release(repository):
    commit_notes(repository)
    commit_notes(
        repository,
        body="Fix for position updates.\n",
        marker="<!-- release-version: 1.2.4 -->\n",
    )
    assert check_notes(repository, "1.2.4").returncode == 0
    assert check_notes(repository, VERSION).returncode != 0


@pytest.mark.parametrize("state", ["modified", "deleted", "staged"])
def test_notes_must_match_head(repository, state):
    commit_notes(repository)
    destination = repository / NOTES
    if state == "deleted":
        destination.unlink()
    else:
        destination.write_text("Uncommitted replacement\n", encoding="utf-8")
        if state == "staged":
            git(repository, "add", NOTES)
    result = check_notes(repository)
    assert result.returncode != 0
    assert "must match the committed release notes" in result.stderr


@pytest.mark.parametrize("version", ["../1.2.3", "v1.2.3", "1.2.3-alpha.1"])
def test_invalid_production_versions_are_rejected(repository, version):
    commit_notes(repository)
    result = check_notes(repository, version)
    assert result.returncode != 0
    assert "MAJOR.MINOR.PATCH" in result.stderr


def release_body_script():
    workflow = (REPO / ".github/workflows/release.yml").read_text(encoding="utf-8")
    step = workflow.split("      - name: Create GitHub Release with runtime assets\n")[
        1
    ]
    run = step.split("        run: |\n", 1)[1]
    lines = []
    for line in run.splitlines():
        if line.strip() and not line.startswith("          "):
            break
        lines.append(line)
    script = textwrap.dedent("\n".join(lines))
    body = script.split("\nmapfile -t ASSETS", 1)[0]
    assert body != script, "Could not isolate release body generation"
    return body + '\ncat "${BODY_FILE}"\n'


def assemble_body(repo, mode):
    return subprocess.run(
        ["bash", "-c", release_body_script()],
        cwd=repo,
        env=dict(
            os.environ,
            TMPDIR=str(repo),
            RELEASE_MODE=mode,
            VERSION=VERSION,
            VCPKG_REGISTRY_BASELINE="abc123",
        ),
        capture_output=True,
        text=True,
    )


def test_production_body_contains_authored_notes_and_installation_metadata(tmp_path):
    path = tmp_path / NOTES
    path.write_text(MARKER + BODY, encoding="utf-8")

    result = assemble_body(tmp_path, "production")

    assert result.returncode == 0, result.stderr
    assert result.stdout.startswith(BODY + "\n\n")
    assert "release-version:" not in result.stdout
    assert f"https://pypi.org/project/openpit/{VERSION}/" in result.stdout
    assert "abc123" in result.stdout


def test_production_body_cannot_be_built_without_notes(tmp_path):
    result = assemble_body(tmp_path, "production")
    assert result.returncode != 0
    assert result.stdout == ""


def test_dry_run_keeps_staging_body_without_production_notes(tmp_path):
    result = assemble_body(tmp_path, "dry-run")
    assert result.returncode == 0, result.stderr
    assert (
        "Draft release used only to validate authentic asset downloads."
        in result.stdout
    )
    assert "Install from package registries" not in result.stdout


@pytest.fixture
def release_workspace(repository):
    git(repository, "config", "core.hooksPath", "/dev/null")
    git(repository, "config", "tag.gpgsign", "false")
    shutil.copyfile(REPO / "justfile", repository / "justfile")
    config = (REPO / "Cargo.toml").read_text(encoding="utf-8")
    release_config = config.split("[workspace.metadata.release]", 1)[1].split(
        "[profile.release]", 1
    )[0]
    (repository / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/*"]\nresolver = "2"\n'
        + "[workspace.metadata.release]"
        + release_config,
        encoding="utf-8",
    )
    crate_config = (REPO / "crates/openpit/Cargo.toml").read_text(encoding="utf-8")
    hook = next(
        line
        for line in crate_config.splitlines()
        if line.startswith("pre-release-hook =")
    )
    for name in ("openpit", "helper"):
        crate = repository / "crates" / name
        (crate / "src").mkdir(parents=True)
        (crate / "src/lib.rs").write_text("pub fn example() {}\n", encoding="utf-8")
        manifest = (
            f'[package]\nname = "release-notes-fixture-{name}"\n'
            'version = "0.1.0"\nedition = "2021"\npublish = false\n'
        )
        if name == "openpit":
            manifest += "[package.metadata.release]\n" + hook + "\n"
        (crate / "Cargo.toml").write_text(manifest, encoding="utf-8")
    (repository / ".gitignore").write_text("target/\n", encoding="utf-8")
    (repository / "README.md").write_text("Test workspace\n", encoding="utf-8")
    subprocess.run(
        ["cargo", "generate-lockfile", "--offline"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
    )
    git(repository, "add", ".")
    git(repository, "commit", "-m", "Add test workspace")
    return repository


def run_cargo_release(repo, *, execute=True, version="0.1.1"):
    prepared = repo.parent / (repo.name + "-prepared-notes.md")
    prepared.write_text(
        f"<!-- release-version: {version} -->\n" + BODY, encoding="utf-8"
    )
    command = ["cargo", "release", "patch", "--no-confirm"]
    if execute:
        command.append("--execute")
    return subprocess.run(
        command,
        cwd=repo,
        env=dict(
            os.environ,
            OPENPIT_RELEASE_NOTES_FILE=str(prepared),
            CARGO_NET_OFFLINE="true",
            CARGO_TARGET_DIR=str(repo / "target"),
        ),
        capture_output=True,
        text=True,
    )


@pytest.mark.parametrize("existing_notes", [False, True])
def test_cargo_release_commits_versions_and_notes_once(
    release_workspace, existing_notes
):
    repo = release_workspace
    if existing_notes:
        commit_notes(repo, marker="<!-- release-version: 0.1.0 -->\n")
    before = int(git(repo, "rev-list", "--count", "HEAD").stdout)

    result = run_cargo_release(repo)

    assert result.returncode == 0, result.stderr
    assert int(git(repo, "rev-list", "--count", "HEAD").stdout) == before + 1
    assert git(repo, "log", "-1", "--format=%s").stdout.strip() == "Release 0.1.1"
    assert (
        git(repo, "rev-parse", "v0.1.1^{commit}").stdout
        == git(repo, "rev-parse", "HEAD").stdout
    )
    assert (
        git(repo, "show", f"HEAD:{NOTES}").stdout
        == "<!-- release-version: 0.1.1 -->\n" + BODY
    )
    changed = git(
        repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"
    ).stdout.splitlines()
    assert NOTES in changed
    assert "crates/openpit/Cargo.toml" in changed
    assert "crates/helper/Cargo.toml" in changed
    assert git(repo, "status", "--porcelain").stdout == ""


def test_cargo_dry_run_does_not_copy_stage_commit_or_tag_notes(release_workspace):
    repo = release_workspace
    before = git(repo, "rev-parse", "HEAD").stdout

    result = run_cargo_release(repo, execute=False)

    assert result.returncode == 0, result.stderr
    assert git(repo, "rev-parse", "HEAD").stdout == before
    assert not (repo / NOTES).exists()
    assert git(repo, "tag", "--list").stdout == ""
    assert git(repo, "status", "--porcelain").stdout == ""


def test_cargo_release_rejects_notes_for_another_version(release_workspace):
    repo = release_workspace
    before = git(repo, "rev-parse", "HEAD").stdout

    result = run_cargo_release(repo, version="0.1.2")

    assert result.returncode != 0
    assert "must start with <!-- release-version: 0.1.1 -->" in result.stderr
    assert git(repo, "rev-parse", "HEAD").stdout == before
    assert not (repo / NOTES).exists()
    assert git(repo, "tag", "--list").stdout == ""


def test_cargo_release_preserves_dirty_tree_rejection(release_workspace):
    repo = release_workspace
    before = git(repo, "rev-parse", "HEAD").stdout
    (repo / "README.md").write_text("Uncommitted changes\n", encoding="utf-8")

    result = run_cargo_release(repo)

    assert result.returncode != 0
    assert git(repo, "rev-parse", "HEAD").stdout == before
    assert not (repo / NOTES).exists()
    assert git(repo, "tag", "--list").stdout == ""
