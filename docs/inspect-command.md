# Inspect Command - Analyzing Soroban Contracts

The `inspect` command provides a way to analyze Soroban contract WASM files without executing them. It displays contract metadata, exported functions, and module statistics.

## Basic Usage

```bash
soroban-debug inspect --contract mycontract.wasm
```

## Key Features

### 1. Display Exported Functions

View all exported functions with their signatures:

```bash
soroban-debug inspect --contract mycontract.wasm --functions
```

**Output (Pretty Format):**

```
Function    Signature
─────────  ────────────────────────────
initialize (admin: Address)
get_value  () -> i64
set_value  (new_val: i64)
```

### 2. Machine-Readable JSON Output

Export function signatures as JSON for integration with tools, CI/CD pipelines, or IDE extensions:

```bash
soroban-debug inspect --contract mycontract.wasm --functions --format json
```

**Output (JSON Format):**

```json
{
  "schema_version": "1.0.0",
  "command": "inspect",
  "status": "success",
  "result": {
    "contract": "mycontract.wasm",
    "size_bytes": 12345,
    "types": 12,
    "functions": 25,
    "exports": 5,
    "exported_functions": [
      {
        "name": "initialize",
        "params": ["admin: Address"],
        "return_type": "()"
      }
    ]
  },
  "error": null
}
```

### 3. Contract Metadata and Module Statistics

Display full contract information including metadata and section breakdown:

```bash
soroban-debug inspect --contract mycontract.wasm
```

This shows:

- File size and statistics
- Module information (type count, function count, export count)
- WASM section breakdown with sizes
- Exported functions with signatures
- Embedded contract metadata (version, SDK version, build date, etc.)

### 4. Full Report as JSON

Get the complete report in JSON format:

```bash
soroban-debug inspect --contract mycontract.wasm --format json
```

## Command Options

| Option                        | Description                                | Default  |
| ----------------------------- | ------------------------------------------ | -------- |
| `-c, --contract <CONTRACT>`   | Path to the contract WASM file             | Required |
| `--functions`                 | Show only exported functions               | Off      |
| `--metadata`                  | Show only contract metadata                | Off      |
| `--format <FORMAT>`           | Output format: `pretty` or `json`          | `pretty` |
| `--expected-hash <HASH>`      | Verify SHA-256 hash matches                | Optional |
| `--dependency-graph <FORMAT>` | Show dependency graph (`dot` or `mermaid`) | Optional |

## Use Cases

### CI/CD Integration

Validate exported functions match expected contract interface:

```bash
soroban-debug inspect --contract build/mycontract.wasm --functions --format json | \
  jq '.exported_functions | length'
```

### IDE Extension Integration

Generate function signatures for IDE autocompletion:

```bash
soroban-debug inspect --contract mycontract.wasm --functions --format json | \
  jq -r '.exported_functions[] | "\(.name)(\(.params | map(.name + ": " + .type) | join(", ")))"'
```

### Contract Documentation

Generate markdown documentation of contract functions:

```bash
soroban-debug inspect --contract mycontract.wasm --functions --format json | \
  jq -r '.exported_functions[] | "- **\(.name)**(\(.params | map(.name + ": " + .type) | join(", ")))"'
```

## JSON Schema

The JSON output for functions follows this schema:

```json
{
  "schema_version": "1.0.0",
  "command": "inspect",
  "status": "success|error",
  "result": "object|null",
  "error": {
    "message": "string"
  }
}
```
