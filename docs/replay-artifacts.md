# Replay Artifact Manifests

When `soroban-debug run` is used with `--trace-output`, the debugger now writes a replay manifest sidecar next to the trace file.

Example:

- `trace.json`
- `trace.manifest.json`

The manifest groups replay-related files so CI systems, bug reports, and other artifact consumers can see which files belong together without opening each file first.

The manifest currently records:

- the exported execution trace
- the contract WASM used for the run
- the network snapshot, when provided
- imported or exported storage files, when provided
- saved command output, when provided
- generated reproduction tests, when provided

The manifest is JSON and is intended as descriptive metadata for artifact bundles. Replay itself still uses the trace JSON as its primary input.
