#!/bin/bash
set -e # Exit immediately if a command fails

echo "--- 🛠️  Running Local CI Gates ---"

echo "1. Checking Rust Formatting..."
cargo fmt --all -- --check

echo "2. Running Clippy Lints..."
cargo clippy --all-targets --all-features -- -D warnings

echo "3. Running Rust Tests..."
cargo test

echo "4. Running VS Code Extension Tests..."
cd extensions/vscode && npm run test && cd ../..

echo "5. Verifying Manpages..."
bash scripts/check_manpages.sh

echo "======================================="
echo "✅ All local CI gates passed!"
echo "======================================="