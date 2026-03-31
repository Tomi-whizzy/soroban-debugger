#!/bin/bash
set -e
WASM_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/wasm"
CONTRACTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/contracts"
WORKSPACE_TARGET_DIR="${CONTRACTS_DIR}/target/wasm32-unknown-unknown/release"

mkdir -p "$WASM_DIR"

for dir in "${CONTRACTS_DIR}"/*/; do
    if [ -d "$dir" ] && [ -f "${dir}Cargo.toml" ]; then
        name=$(basename "$dir")
        package_name=$(sed -n 's/^name = "\(.*\)"/\1/p' "${dir}Cargo.toml" | head -n 1)
        
        if [ -z "${package_name}" ]; then
            echo "Failed to determine package name for ${name}"
            continue
        fi

        WASM_OUT="$WASM_DIR/${name}.wasm"
        NEEDS_BUILD=true

        if [ -f "$WASM_OUT" ]; then
            # Check if any source file or Cargo.toml is newer than the existing WASM
            if [ -z "$(find "$dir" -type f \( -name "*.rs" -o -name "Cargo.toml" \) -newer "$WASM_OUT")" ]; then
                NEEDS_BUILD=false
            fi
        fi

        if [ "$NEEDS_BUILD" = true ]; then
            echo "Building $name..."
            (cd "$dir" && cargo build --release --target wasm32-unknown-unknown)
            cp "${WORKSPACE_TARGET_DIR}/${package_name//-/_}.wasm" "$WASM_OUT"
        else
            echo "Skipping $name (no changes detected)."
        fi
    fi
done
