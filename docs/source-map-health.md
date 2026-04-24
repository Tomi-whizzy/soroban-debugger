# Source Map Health Diagnostics

The Soroban Debugger provides a health diagnostic report to help developers understand the quality of the source-to-WASM mappings in their contract artifacts. This is crucial for ensuring that breakpoints and source-level stepping work reliably.

## Interpreting the Report

When running `soroban-debug inspect` or using the VS Code "Diagnose Source Maps" command, you will see a "Source Map (DWARF)" section:

- **Mapped Executable Lines**: The total number of distinct WASM offsets that have been successfully mapped back to a source file, line, and column.
- **DWARF Units Processed**: The number of Compilation Units found in the DWARF debug information. Each unit typically corresponds to a source file or module.
- **Units with Mappings**: How many of those units contained valid line number programs.
- **Mapping Coverage**: A percentage representing the ratio of units with mappings to total units.
    - **90%+ (Green)**: Excellent. Most of your code should be debuggable at the source level.
    - **50%-89% (Yellow)**: Degraded. Some modules (possibly dependencies or specific macros) are missing debug info.
    - **<50% (Red)**: Poor. Large portions of the contract will only be debuggable via WASM instructions.

## Common Issues

### "DWARF unit is missing a line program"
This usually happens when a dependency was compiled without debug symbols or when the `[profile.release]` in your `Cargo.toml` is missing `debug = true`.

### "Failed to load DWARF sections"
The WASM file might have been stripped of its custom sections. Ensure you are using the `contract.wasm` from `target/wasm32-unknown-unknown/release/` and not a "minimized" version.

### Low Coverage in Release Builds
Even with `debug = true`, optimizations can merge or delete code, leading to fragmented mappings. For the best debugging experience, use a `debug` profile or reduce optimization levels.
