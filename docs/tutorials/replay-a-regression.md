# Tutorial: Replay a Regression

This tutorial covers the `replay` command, which allows you to perfectly reproduce a past execution using an exported trace file.

## The Goal
Re-run a specific execution state to debug a failure or verify a fix without manually reconstructing arguments and storage.

## Step 1: Obtain a trace
You need an execution trace from a previous run (or a bug report):

```bash
soroban-debug run \
  --contract buggy.wasm \
  --function process \
  --args '["edge_case"]' \
  --trace-output crash_trace.json
```

## Step 2: Replay the trace
Use the `replay` command to load the exact arguments and storage state from the trace:

```bash
soroban-debug replay --trace crash_trace.json
```

## Step 3: Interactive replay
If you want to step through the replayed execution to find the bug, add the `--interactive` flag:

```bash
soroban-debug replay --trace crash_trace.json --interactive
```