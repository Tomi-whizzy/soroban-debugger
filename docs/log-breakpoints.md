# Log-Only Breakpoints (Log Points)

Log-only breakpoints, commonly known as **logpoints**, allow you to instrument contract execution with contextual logging **without pausing** the debugger. This is particularly useful when you want to trace execution flow, monitor variable changes, or collect metrics during long-running operations without interrupting the execution flow.

## When to Use Log Points

Use log points instead of regular breakpoints when:

- **Tracing execution flow** during long runs where stopping at each point would be tedious
- **Monitoring variable changes** across multiple iterations without manual continuation
- **Collecting execution metrics** like step counts, function call frequencies, or state transitions
- **Debugging production-like scenarios** where you need observability but minimal intrusion
- **Understanding contract behavior** by logging entry/exit points of multiple functions

## CLI Usage

### Basic Syntax

Log points are specified using the `--log-point` flag with the format `FUNCTION=MESSAGE`:

```bash
soroban-debug run \
  --contract token.wasm \
  --function transfer \
  --log-point "transfer=Transfer function called"
```

### Multiple Log Points

You can set multiple log points in a single command:

```bash
soroban-debug run \
  --contract token.wasm \
  --function transfer \
  --log-point "transfer=Entering transfer" \
  --log-point "mint=Entering mint" \
  --log-point "burn=Entering burn"
```

### Combining with Regular Breakpoints

Log points can be used alongside regular breakpoints. Regular breakpoints will pause execution, while log points will only log:

```bash
soroban-debug run \
  --contract token.wasm \
  --function transfer \
  --breakpoint transfer \
  --log-point "mint=Mint called at step {step_count}"
```

## Template Variables

Log point messages support template variables that are interpolated at runtime:

| Variable | Description | Example Output |
|----------|-------------|----------------|
| `{function}` | Current function name | `transfer` |
| `{args}` | Function arguments as JSON | `["addr1", 100]` |
| `{step_count}` | Current execution step count | `42` |

### Examples

```bash
# Log with function name
--log-point "transfer=Called function: {function}"

# Log with step count
--log-point "transfer=Step {step_count}: Transfer executed"

# Log with arguments
--log-point "transfer=Transfer with args: {args}"

# Combined template
--log-point "transfer=[Step {step_count}] {function} called with {args}"
```

## Output Format

When a log point is hit, the debugger outputs a formatted message:

```
[LOG @transfer] Transfer executed at step 42
```

The format is: `[LOG @<function_name>] <interpolated_message>`

Log point hits are also recorded in the structured tracing output (visible with `--verbose` or when RUST_LOG is configured).

## Comparison: Log Points vs Regular Breakpoints

| Feature | Regular Breakpoint | Log Point |
|---------|-------------------|-----------|
| **Pauses execution** | Yes | No |
| **Logs message** | Optional | Always |
| **Supports conditions** | Yes | No (always logs) |
| **Supports hit conditions** | Yes | No (always logs) |
| **Use case** | Stop and inspect | Trace and continue |
| **CLI flag** | `--breakpoint` | `--log-point` |

## Performance Considerations

- **Minimal overhead**: Log points have very low performance impact since they don't pause execution
- **String interpolation**: Template variables are interpolated on each hit, which adds minimal CPU overhead
- **I/O impact**: Console output (`println!`) may have noticeable impact if the log point is hit thousands of times per second
- **Tracing overhead**: Structured logging via `tracing::info!` is asynchronous and has minimal impact

### Best Practices

1. **Avoid high-frequency log points** in tight loops unless necessary
2. **Use concise messages** to reduce I/O overhead
3. **Combine with `--quiet`** flag if you only want structured logs, not console output
4. **Use regular breakpoints** when you need to inspect state in detail

## Examples

### Example 1: Trace Function Calls

Track all function calls in a token contract:

```bash
soroban-debug run \
  --contract token.wasm \
  --function mint \
  --log-point "mint=Minting tokens" \
  --log-point "transfer=Transferring tokens" \
  --log-point "burn=Burning tokens"
```

### Example 2: Monitor Execution Progress

See how many steps each function takes:

```bash
soroban-debug run \
  --contract complex.wasm \
  --function execute \
  --log-point "execute=Starting at step {step_count}"
```

### Example 3: Trace with Arguments

Log function arguments for debugging:

```bash
soroban-debug run \
  --contract token.wasm \
  --function transfer \
  --args '["GABC...", 1000]' \
  --log-point "transfer=Transfer called with: {args}"
```

## Interactive Mode

Log points are also supported in interactive mode:

```bash
soroban-debug interactive \
  --contract token.wasm \
  --function transfer \
  --log-point "transfer=Transfer invoked"
```

## TUI Dashboard

The TUI dashboard also supports log points:

```bash
soroban-debug tui \
  --contract token.wasm \
  --function transfer \
  --log-point "transfer=Transfer executed"
```

Log point messages will appear in the output panel without interrupting the dashboard UI.

## Implementation Details

- Log points are implemented using the existing breakpoint infrastructure
- The `Breakpoint::log_point()` constructor creates breakpoints with `log_message` set
- During execution, `should_break_with_context()` returns `(false, Some(message))` for log points
- The engine logs the message and continues execution without pausing
- Template interpolation is handled by `DebugStateEvaluator::interpolate_log()`

## Troubleshooting

### Log point not triggering

- Verify the function name matches exactly (case-sensitive)
- Check that the function is actually being called during execution
- Use `--verbose` to see if the log point is being registered

### Invalid format warning

If you see: `Warning: Invalid log point format '...', expected FUNCTION=MESSAGE`

- Ensure you're using the `=` separator
- Don't include spaces around the `=`
- Quote the entire specification if it contains spaces: `"transfer=my message"`

### No output visible

- Check if you're using `--quiet` flag (suppresses some output)
- Verify RUST_LOG environment variable is configured if expecting structured logs
- The log point may not be hit if the function isn't called

## Future Enhancements

Potential future improvements to log points:

- Conditional log points (only log when condition is met)
- Hit conditions for log points (log every Nth hit)
- Custom variable interpolation (access storage values, etc.)
- Log point groups (enable/disable sets of log points)
- Output to file instead of console
- Rate limiting for high-frequency log points
