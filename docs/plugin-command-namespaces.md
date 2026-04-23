# Plugin Command Namespace Policy

## Problem

Plugins can provide custom CLI commands and output formatters, but name collisions between plugins make behavior unpredictable.

## Solution

Plugin command and formatter names are now resolved deterministically.

### Precedence order

1. Core commands always win.
2. Plugin names are sorted deterministically by manifest name.
3. If the manifest name is missing or empty, the plugin file name is used as a fallback.

### Normalization rules

- Plugin command names are normalized by trimming whitespace and lowercasing.
- Formatter names are normalized the same way.

### Conflict behavior

- The first plugin in deterministic order wins for execution.
- All providers are recorded in a conflict map so collisions are visible.
- Warnings are emitted once during plugin loading or registry initialization, not during every command execution.

### Example warning

```
Plugin command collision: 'foo' winner: plugin_a ignored: plugin_b, plugin_c
```

## Best practice

Use a namespaced command style where practical:

```text
plugin_name:command_name
```

This reduces collision risk and makes it easier to understand where a command comes from.

## Notes

- No new plugin API is required for this behavior.
- Existing plugin commands continue to work.
- The registry exposes conflicts via `command_conflicts()` and `formatter_conflicts()`.
