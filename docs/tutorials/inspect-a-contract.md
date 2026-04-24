# Tutorial: Inspect a Contract

This tutorial covers the `inspect` command, used to view contract metadata and exports without executing any code.

## The Goal
Understand what functions a contract exposes and review its embedded metadata.

## Step 1: Basic inspection
Run the `inspect` command on your compiled WASM:

```bash
soroban-debug inspect --contract my_contract.wasm
```

This prints a human-readable summary of the contract's size, sections, exported functions, and signatures.

## Step 2: JSON output for tooling
If you are writing a script or integrating with an IDE, output the results as JSON:

```bash
soroban-debug inspect --contract my_contract.wasm --format json
```

## Step 3: DWARF Source Map Diagnostics
Check if your contract has valid source mapping information for debugging:

```bash
soroban-debug inspect --contract my_contract.wasm --source-map-diagnostics
```

This tells you if the WASM was built in release mode with stripped symbols or if it retains DWARF metadata.