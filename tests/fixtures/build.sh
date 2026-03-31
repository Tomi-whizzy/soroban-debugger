#!/bin/bash
# Build script to compile all test fixture contracts to WASM and refresh manifest.json.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACTS_DIR="${SCRIPT_DIR}/contracts"
WASM_DIR="${SCRIPT_DIR}/wasm"
MANIFEST_PATH="${SCRIPT_DIR}/manifest.json"
RELEASE_TARGET_DIR="${CONTRACTS_DIR}/target/wasm32-unknown-unknown/release"
DEBUG_TARGET_DIR="${CONTRACTS_DIR}/target/wasm32-unknown-unknown/release-debug"

fixture_exports_json() {
    case "$1" in
        always_panic) printf '["panic"]' ;;
        budget_heavy) printf '["heavy"]' ;;
        counter) printf '["get","increment"]' ;;
        cross_contract) printf '["call"]' ;;
        echo) printf '["echo"]' ;;
        same_return) printf '["same"]' ;;
        *)
            echo "Unknown fixture export set for '$1'" >&2
            exit 1
            ;;
    esac
}

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        echo "Could not find sha256sum or shasum" >&2
        exit 1
    fi
}

append_manifest_entry() {
    local contract_name="$1"
    local release_hash="$2"
    local debug_hash="$3"
    local exports_json
    exports_json="$(fixture_exports_json "${contract_name}")"

    if [ "${FIRST_ENTRY}" = false ]; then
        printf ',\n' >> "${MANIFEST_PATH}"
    fi
    FIRST_ENTRY=false

    cat >> "${MANIFEST_PATH}" <<EOF
    {
      "name": "${contract_name}",
      "exports": ${exports_json},
      "source": {
        "contract_dir": "tests/fixtures/contracts/${contract_name}",
        "lib_rs": "tests/fixtures/contracts/${contract_name}/src/lib.rs"
      },
      "artifacts": {
        "release": {
          "path": "tests/fixtures/wasm/${contract_name}.wasm",
          "sha256": "${release_hash}"
        },
        "debug": {
          "path": "tests/fixtures/wasm/${contract_name}_debug.wasm",
          "sha256": "${debug_hash}"
        }
      }
    }
EOF
}

if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "Error: wasm32-unknown-unknown target not installed."
    echo "Install it with: rustup target add wasm32-unknown-unknown"
    exit 1
fi

mkdir -p "${WASM_DIR}"

echo "Building test fixture contracts..."

printf '{\n  "version": 1,\n  "fixtures": [\n' > "${MANIFEST_PATH}"
FIRST_ENTRY=true

for contract_dir in "${CONTRACTS_DIR}"/*/; do
    if [ ! -f "${contract_dir}Cargo.toml" ]; then
        continue
    fi

    contract_name="$(basename "${contract_dir}")"
    echo "  Building ${contract_name}..."

    (
        cd "${contract_dir}"
        cargo build --release --target wasm32-unknown-unknown
        cargo build --profile release-debug --target wasm32-unknown-unknown
    )

    package_name="$(sed -n 's/^name = "\(.*\)"/\1/p' "${contract_dir}Cargo.toml" | head -n 1)"
    if [ -z "${package_name}" ]; then
        echo "Failed to determine package name for ${contract_name}" >&2
        exit 1
    fi

    release_src="${RELEASE_TARGET_DIR}/${package_name//-/_}.wasm"
    debug_src="${DEBUG_TARGET_DIR}/${package_name//-/_}.wasm"
    release_dest="${WASM_DIR}/${contract_name}.wasm"
    debug_dest="${WASM_DIR}/${contract_name}_debug.wasm"

    if [ ! -f "${release_src}" ]; then
        echo "Failed to find release WASM output for ${contract_name}" >&2
        exit 1
    fi
    if [ ! -f "${debug_src}" ]; then
        echo "Failed to find debug WASM output for ${contract_name}" >&2
        exit 1
    fi

    cp "${release_src}" "${release_dest}"
    cp "${debug_src}" "${debug_dest}"

    append_manifest_entry \
        "${contract_name}" \
        "$(sha256_file "${release_dest}")" \
        "$(sha256_file "${debug_dest}")"
done

printf '\n  ]\n}\n' >> "${MANIFEST_PATH}"

echo ""
echo "All contracts built successfully."
echo "WASM files are in: ${WASM_DIR}"
echo "Manifest refreshed at: ${MANIFEST_PATH}"
