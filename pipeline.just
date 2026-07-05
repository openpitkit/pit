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

# CI/CD pipeline-only recipes (release + pit-go mirror). Invoked from
# .github/workflows/* via: just --justfile pipeline.just <recipe>
# Not for local development. This file imports the dev justfile, so all
# dev recipes are available here too.

import 'justfile'

# Verify all crate manifest versions match the release version.
check-cargo-versions version:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION="{{ version }}"
    for MANIFEST in crates/openpit/Cargo.toml crates/openpit-derive/Cargo.toml crates/openpit-ffi/Cargo.toml bindings/python/Cargo.toml; do
      CARGO_VERSION="$(grep -m1 '^version\s*=' "${MANIFEST}" | sed 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/')"
      if [[ "${CARGO_VERSION}" != "${VERSION}" ]]; then
        echo "::error::Tag version ${VERSION} does not match ${MANIFEST} version ${CARGO_VERSION}"
        exit 1
      fi
      echo "${MANIFEST}: ${CARGO_VERSION} OK"
    done
# Validate the tag is a monotonic version bump over the previous tag.
check-version-bump tag:
    #!/usr/bin/env bash
    set -euo pipefail

    CURRENT_TAG="{{ tag }}"
    CURRENT_VERSION="${CURRENT_TAG#v}"
    PREVIOUS_TAG="$(
      git tag -l 'v*' \
        | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' \
        | grep -vx "${CURRENT_TAG}" \
        | sort -V \
        | tail -n 1 || true
    )"

    if [[ -z "${PREVIOUS_TAG}" ]]; then
      echo "No previous release tag found, skipping monotonic check"
      exit 0
    fi

    PREVIOUS_VERSION="${PREVIOUS_TAG#v}"

    IFS='.' read -r CUR_MAJOR CUR_MINOR CUR_PATCH <<< "${CURRENT_VERSION}"
    IFS='.' read -r PREV_MAJOR PREV_MINOR PREV_PATCH <<< "${PREVIOUS_VERSION}"

    if (( CUR_MAJOR < PREV_MAJOR )); then
      echo "::error::Major version decreased: ${CURRENT_VERSION} < ${PREVIOUS_VERSION}"
      exit 1
    fi

    if (( CUR_MAJOR > PREV_MAJOR )); then
      if (( CUR_MINOR != 0 || CUR_PATCH != 0 )); then
        echo "::error::Major version bump requires minor and patch to be 0 (got ${CURRENT_VERSION}, previous ${PREVIOUS_VERSION})"
        exit 1
      fi
      echo "Valid major version bump: ${PREVIOUS_VERSION} -> ${CURRENT_VERSION}"
      exit 0
    fi

    # major equal
    if (( CUR_MINOR < PREV_MINOR )); then
      echo "::error::Minor version decreased: ${CURRENT_VERSION} < ${PREVIOUS_VERSION}"
      exit 1
    fi

    if (( CUR_MINOR > PREV_MINOR )); then
      if (( CUR_PATCH != 0 )); then
        echo "::error::Minor version bump requires patch to be 0 (got ${CURRENT_VERSION}, previous ${PREVIOUS_VERSION})"
        exit 1
      fi
      echo "Valid minor version bump: ${PREVIOUS_VERSION} -> ${CURRENT_VERSION}"
      exit 0
    fi

    # major and minor equal
    if (( CUR_PATCH <= PREV_PATCH )); then
      echo "::error::Patch version must increase: ${CURRENT_VERSION} <= ${PREVIOUS_VERSION}"
      exit 1
    fi

    echo "Valid patch version bump: ${PREVIOUS_VERSION} -> ${CURRENT_VERSION}"
# Ensure crates.io has not already published this version.
check-crate-unpublished version:
    #!/usr/bin/env bash
    set -euo pipefail

    VERSION="{{ version }}"
    for CRATE in openpit-derive openpit; do
      URL="https://crates.io/api/v1/crates/${CRATE}/${VERSION}"
      HTTP_CODE="$(curl -H "User-Agent: openpit-release-ci" --retry 3 --retry-delay 2 -sS -o "/tmp/${CRATE}-crates-version.json" -w '%{http_code}' "${URL}")"

      case "${HTTP_CODE}" in
        200)
          echo "::error::${CRATE} version ${VERSION} already exists on crates.io"
          exit 1
          ;;
        404)
          echo "${CRATE} version ${VERSION} is not published on crates.io yet"
          ;;
        *)
          echo "::error::Failed to query crates.io for ${CRATE} (HTTP ${HTTP_CODE})"
          cat "/tmp/${CRATE}-crates-version.json" || true
          exit 1
          ;;
      esac
    done
# Rewrite the Python package version for a staging (dry-run) release.
apply-staging-version version:
    #!/usr/bin/env bash
    set -euo pipefail
    export STAGING_VERSION="{{ version }}"
    python - <<'PY'
    from pathlib import Path
    import os

    path = Path("bindings/python/Cargo.toml")
    lines = path.read_text().splitlines()
    for index, line in enumerate(lines):
        if line.startswith("version = "):
            lines[index] = f'version = "{os.environ["STAGING_VERSION"]}"'
            break
    else:
        raise SystemExit("version field not found in bindings/python/Cargo.toml")
    path.write_text("\n".join(lines) + "\n")
    PY
# Build Python API docs; warnings are errors.
build-python-docs: python-develop-debug
    cd bindings/python && sphinx-build -b html docs docs/_build -W
# Copy monorepo LICENSE and OWNERS into the Python package.
copy-python-metadata:
    cp LICENSE bindings/python/LICENSE
    cp OWNERS bindings/python/OWNERS
# Resolve the built wheel path; prints the path to stdout.
resolve-wheel version:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION="{{ version }}"
    CANDIDATE_DIRS=(
      "bindings/python/dist"
      "dist"
      "bindings/python/target/wheels"
      "target/wheels"
    )

    WHEELS=()
    for DIR in "${CANDIDATE_DIRS[@]}"; do
      if [[ -d "${DIR}" ]]; then
        while IFS= read -r FILE; do
          WHEELS+=("${FILE}")
        done < <(find "${DIR}" -maxdepth 1 -type f -name "openpit-*.whl" | sort)
      fi
    done

    if [[ ${#WHEELS[@]} -eq 0 ]]; then
      echo "::error::No wheel files found in expected directories" >&2
      exit 1
    fi

    VERSION_MATCH=()
    for FILE in "${WHEELS[@]}"; do
      BASE="$(basename "${FILE}")"
      if [[ "${BASE}" == openpit-"${VERSION}"*".whl" ]]; then
        VERSION_MATCH+=("${FILE}")
      fi
    done

    if [[ ${#VERSION_MATCH[@]} -eq 1 ]]; then
      WHEEL="${VERSION_MATCH[0]}"
    elif [[ ${#VERSION_MATCH[@]} -gt 1 ]]; then
      echo "::error::Expected exactly one wheel for version ${VERSION}, found ${#VERSION_MATCH[@]}" >&2
      printf '%s\n' "${VERSION_MATCH[@]}" >&2
      exit 1
    elif [[ ${#WHEELS[@]} -eq 1 ]]; then
      WHEEL="${WHEELS[0]}"
    else
      echo "::error::Could not uniquely resolve wheel path" >&2
      printf '%s\n' "${WHEELS[@]}" >&2
      exit 1
    fi

    echo "${WHEEL}"
# Verify the installed wheel imports and reports the expected version.
verify-wheel-version mode version base_version:
    #!/usr/bin/env bash
    set -euo pipefail
    export RELEASE_MODE="{{ mode }}"
    export VERSION="{{ version }}"
    export BASE_VERSION="{{ base_version }}"
    python -c "import openpit"
    python - <<'PY'
    import importlib.metadata as metadata
    import os

    version = metadata.version("openpit")
    print(version)

    if os.environ["RELEASE_MODE"] == "production":
        expected = os.environ["VERSION"]
        assert version == expected, f"expected {expected}, got {version}"
    else:
        base_version = os.environ["BASE_VERSION"]
        assert version != base_version, f"expected staging version, got production version {version}"
        assert version.startswith(base_version), f"expected version starting with {base_version}, got {version}"
    PY
# Run the wheel test suite against a pre-installed wheel (no rebuild).
test-wheel:
    just _pytest bindings/python/tests
# Validate wheel metadata with twine.
check-wheel path:
    twine check {{ path }}
# Build the Python source distribution.
build-sdist:
    maturin sdist --manifest-path bindings/python/Cargo.toml --out dist
# Validate that the sdist declares all its License-File entries.
validate-sdist-license:
    #!/usr/bin/env bash
    set -euo pipefail
    python - <<'PY'
    import email
    import tarfile
    from pathlib import Path

    sdists = sorted(Path("dist").glob("*.tar.gz"))
    if len(sdists) != 1:
        raise SystemExit(f"expected exactly one sdist, found {len(sdists)}")

    with tarfile.open(sdists[0], "r:gz") as archive:
        names = set(archive.getnames())
        pkg_info_name = next(
            (
                name
                for name in names
                if name.count("/") == 1 and name.endswith("/PKG-INFO")
            ),
            None,
        )
        if pkg_info_name is None:
            raise SystemExit(f"{sdists[0]} has no top-level PKG-INFO")

        root = pkg_info_name.rsplit("/", 1)[0]
        pkg_info = archive.extractfile(pkg_info_name)
        if pkg_info is None:
            raise SystemExit(f"{pkg_info_name} cannot be read")

        metadata = email.message_from_bytes(pkg_info.read())
        missing = [
            license_file
            for license_file in metadata.get_all("License-File", [])
            if f"{root}/{license_file}" not in names
        ]
        if missing:
            raise SystemExit(
                "sdist declares missing License-File entries: "
                + ", ".join(missing)
            )
    PY
# Validate sdist metadata with twine.
check-sdist:
    twine check dist/*
# Smoke-test installing the sdist into a fresh venv.
smoke-sdist mode version base_version:
    #!/usr/bin/env bash
    set -euo pipefail
    export RELEASE_MODE="{{ mode }}"
    export VERSION="{{ version }}"
    export BASE_VERSION="{{ base_version }}"
    python -m venv /tmp/openpit-sdist-smoke
    source /tmp/openpit-sdist-smoke/bin/activate
    export PYTHON_PATH=/tmp/openpit-sdist-smoke/bin/python
    export PYO3_PYTHON="${PYTHON_PATH}"
    python -m pip install --upgrade pip
    python -m pip install maturin
    python -m pip install dist/*.tar.gz
    python -c "import openpit"
    python - <<'PY'
    import importlib.metadata as metadata
    import os

    version = metadata.version("openpit")
    print(version)

    if os.environ["RELEASE_MODE"] == "production":
        expected = os.environ["VERSION"]
        assert version == expected, f"expected {expected}, got {version}"
    else:
        base_version = os.environ["BASE_VERSION"]
        assert version != base_version, f"expected staging version, got production version {version}"
        assert version.startswith(base_version), f"expected version starting with {base_version}, got {version}"
    PY
# Build the FFI runtime library for a target triple (release).
build-ffi-target-release target:
    #!/usr/bin/env bash
    set -euo pipefail
    rustflags="${RUSTFLAGS:-}"
    rustflags="${rustflags:+${rustflags} }--remap-path-prefix=$(pwd)=."
    case "{{ target }}" in
      *-pc-windows-msvc)
        rustflags="${rustflags} -C target-feature=+crt-static"
        ;;
    esac
    export RUSTFLAGS="${rustflags}"
    cargo build -p openpit-ffi --release --locked --target "{{ target }}"
# Package the built runtime library into dist/ with a sha256 sidecar (unix).
[unix]
package-runtime target goos goarch lib_name:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p dist
    SRC="target/{{ target }}/release/{{ lib_name }}"

    if command -v strip >/dev/null 2>&1; then
      if strip --help 2>&1 | grep -q -- '--strip-unneeded'; then
        strip --strip-unneeded "${SRC}" || true
      else
        strip -x "${SRC}" || true
      fi
    fi

    DST="dist/openpit-ffi--{{ goos }}-{{ goarch }}-{{ lib_name }}"
    cp "${SRC}" "${DST}"
    cp bindings/c/openpit.h dist/openpit.h
    cp LICENSE dist/LICENSE
    cp OWNERS dist/OWNERS

    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "${DST}" | awk '{print $1}' > "${DST}.sha256"
    else
      shasum -a 256 "${DST}" | awk '{print $1}' > "${DST}.sha256"
    fi

[windows]
package-runtime target goos goarch lib_name:
    {{ bootstrap_python }} {{ just_helper }} package-runtime "{{ target }}" "{{ goos }}" "{{ goarch }}" "{{ lib_name }}"

# Package the C++ binding distributable into dist/ with a sha256 sidecar.
# Bundles: public headers, generated C ABI header, CMake config-file package
# (OpenPitConfig, OpenPitConfigVersion, OpenPitTargets, OpenPitDownloadRuntime),
# LICENSE, and OWNERS. The cmake install tree is expected at CPP_INSTALL_DIR.
package-cpp cpp_install_dir:
    #!/usr/bin/env bash
    set -euo pipefail
    VERSION="$(grep -m1 '^version[[:space:]]*=' crates/openpit/Cargo.toml | sed 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/')"
    : "${VERSION:?could not read version from crates/openpit/Cargo.toml}"

    INSTALL_DIR="{{ cpp_install_dir }}"
    ARCHIVE_NAME="openpit-cpp--${VERSION}.tar.gz"
    mkdir -p dist

    tar -czf "dist/${ARCHIVE_NAME}" \
      -C "${INSTALL_DIR}" \
      include \
      lib \
      LICENSE \
      OWNERS

    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "dist/${ARCHIVE_NAME}" | awk '{print $1}' > "dist/${ARCHIVE_NAME}.sha256"
    else
      shasum -a 256 "dist/${ARCHIVE_NAME}" | awk '{print $1}' > "dist/${ARCHIVE_NAME}.sha256"
    fi
    echo "Packaged dist/${ARCHIVE_NAME}"
# Run Go tests in an in-worktree split dir without the embedded-runtime env.
test-go-split dir:
    cd {{ dir }} && env -u OPENPIT_RUNTIME_LIBRARY_PATH -u OPENPIT_RUNTIME_CACHE_DIR go test ./...
