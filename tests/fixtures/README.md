# Test Fixture Contracts

This directory contains Soroban contract fixtures used by the test suite.
`manifest.json` is the checked-in source of truth for fixture exports, hashes, source paths, and artifact locations.

## Contracts

- `counter` - Simple counter contract with `increment` and `get`
- `echo` - Echo contract that returns its input unchanged
- `always_panic` - Contract that always panics, useful for error testing
- `budget_heavy` - Contract with budget-intensive operations for budget testing
- `cross_contract` - Contract that calls other contracts for cross-contract call testing
- `same_return` - Contract with divergent branches that intentionally return the same value

## Building

To rebuild all fixture artifacts and refresh the manifest:

Linux/macOS:
```bash
./build.sh
```

Windows:
```powershell
.\build.ps1
```

Both scripts build release and `release-debug` WASM artifacts under `tests/fixtures/wasm/` and then rewrite `tests/fixtures/manifest.json` with the expected exports, source paths, and SHA-256 hashes for every generated artifact.

## Usage in Tests

```rust
#[path = "fixtures/mod.rs"]
mod fixtures;

#[test]
fn test_with_counter() {
    let wasm_path = fixtures::get_fixture_path(fixtures::names::COUNTER);
    let wasm_bytes = std::fs::read(&wasm_path).unwrap();
    let source_path = fixtures::source_path(fixtures::names::COUNTER);

    assert!(source_path.exists());
    assert!(!wasm_bytes.is_empty());
}
```

Use `fixtures::artifact_path(name, "debug")` when a test needs the debug-info-preserving fixture.

## Manifest Shape

The manifest is JSON and includes, for each fixture:

- the exported contract functions
- the source contract directory and `src/lib.rs` path
- the release artifact path and SHA-256 hash
- the debug artifact path and SHA-256 hash when debug fixtures have been generated

## Structure

```text
tests/fixtures/
|-- contracts/
|-- manifest.json
|-- wasm/
|-- build.sh
|-- build.ps1
`-- README.md
```
