# Soroban Debugger — developer convenience targets
#
# Targets:
#   regen-man   Regenerate all man pages from current CLI source
#   check-man   Verify committed man pages match generated output (used in CI)
#   fmt         Check Rust formatting
#   lint        Run Rust clippy lints
#   test-rust   Run Rust backend tests
#   test-vscode Run VS Code extension tests
#   ci-local    Run all practical gates developers must satisfy before pushing

.PHONY: all build fmt lint test-rust test-vscode ci-local clean regen-man check-man

all: build

build:
	cargo build
	cd extensions/vscode && npm install && npm run build

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

test-rust:
	cargo test

test-vscode:
	cd extensions/vscode && npm install && npm run test

# Regenerate all man pages from current CLI source.
# Run after any CLI flag, subcommand, or help text change, then commit the .1 files.
regen-man:
	@echo "Regenerating man pages..."
	cargo build --quiet
	@echo "Man pages updated in man/man1/ — remember to commit the .1 files."

# Verify committed man pages match generated output.
# Exits non-zero with a diff if drift is detected.
check-man:
	@bash scripts/check_manpages.sh

# The single local entrypoint for developers
ci-local: fmt lint test-rust test-vscode check-man
	@echo "======================================="
	@echo "✅ All local CI gates passed successfully!"
	@echo "======================================="

clean:
	cargo clean
	rm -rf extensions/vscode/node_modules extensions/vscode/dist