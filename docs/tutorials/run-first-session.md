# Tutorial: Run Your First Debug Session

This tutorial covers the `run` command, which is the most common way to execute and debug a single contract function from the CLI.

## The Goal
Execute a contract function with specific arguments and inspect the execution results, including budget and storage changes.

## Prerequisites
- A compiled Soroban contract (e.g., `token.wasm`).

## Step 1: Execute a function
Use the `run` command to execute a function. We'll pass arguments as a JSON array using the `--args` flag.

```bash
soroban-debug run \
  --contract token.wasm \
  --function transfer \
  --args '["user1", "user2", 100]'
```

## Step 2: Understand the output
The debugger will output a summary of the execution:
- **Result:** The return value of the function (e.g., `Ok(())`).
- **Budget:** CPU instructions and Memory bytes consumed.
- **Storage Diff:** Any state changes made by the contract.

## Step 3: Export a trace
If you want to save the execution details for later analysis, you can export a full trace to a JSON file using `--trace-output`:

```bash
soroban-debug run --contract token.wasm --function transfer --args '["user1", "user2", 100]' --trace-output trace.json
```
This trace can later be used with the `compare` or `replay` commands.