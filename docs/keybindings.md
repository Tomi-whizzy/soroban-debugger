# Configurable Keybindings

Soroban Debugger provides a mechanism to override the default single-character or short-string aliases used within its interactive environments (like the REPL and basic TUI). This allows you to adapt the interface to your preferred workflow or existing terminal habits (e.g., matching GDB, LLDB, or Vim bindings).

## Configuration

Keybindings are configured via the `.soroban-debug.toml` file in your project root under the `[keybindings]` table.

### Supported Keybindings

| Setting         | Default | Description                            |
|-----------------|---------|----------------------------------------|
| `step`          | `s`     | Step the execution to the next point.  |
| `continue_exec` | `c`     | Continue execution until complete or next breakpoint. |
| `inspect`       | `i`     | Display the current execution state, stack, and values. |
| `quit`          | `q`     | Exit the debugger REPL/TUI.            |

### Example Config (`.soroban-debug.toml`)

```toml
[keybindings]
step = "n"           # Change step/next to 'n'
continue_exec = "r"  # Change continue/run to 'r'
inspect = "p"        # Change inspect/print to 'p'
quit = "x"           # Change quit to 'x'
```

## Behavior

1. **Safe Defaults Preserved**: Customizing aliases will not break the full word commands (e.g., `step`, `continue`, `inspect`, `quit`, `exit` will always be recognized alongside your custom binding).
2. **Help Menu Updating**: When you invoke the `help` command in the interactive session, your newly configured aliases will dynamically reflect in the UI readout.

> **Note on Dashboard (Ratatui TUI)**
> 
> This configuration specifically overrides the text-based REPL interface shortcuts (e.g., commands parsed via `stdin`). Future iterations will extend custom keystroke definitions (Ctrl/Alt bindings) into the full-screen visual dashboard.