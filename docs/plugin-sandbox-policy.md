# Plugin Sandbox Policy

The Soroban Debugger provides a configurable sandbox policy to control what dynamically loaded plugins are allowed to do. While the Trust Policy governs *which* plugins can be loaded (via signatures and allowlists), the Sandbox Policy governs *what* those plugins are permitted to access or register once loaded.

## Configuration

Sandbox policies are defined in your `.soroban-debug.toml` configuration file.

```toml
[plugins.sandbox]
allow_file_read = true
allow_file_write = false
allow_network = false
max_execution_duration_ms = 5000
allow_command_registration = true
```

## Available Policies

| Policy | Default | Description |
|---|---|---|
| `allow_file_read` | `true` | Allows plugins to read from the local filesystem. |
| `allow_file_write` | `false` | Allows plugins to write to the local filesystem. |
| `allow_network` | `false` | Allows plugins to make outbound network requests. |
| `max_execution_duration_ms` | `5000` | Maximum time (in milliseconds) a plugin is allowed to spend handling a single event or command before it is considered to have timed out and is isolated. |
| `allow_command_registration` | `true` | Allows plugins to register new custom CLI commands. If set to `false`, plugins that declare `provides_commands = true` in their manifest will be rejected at load time. |

## Enforcing Capabilities

Some policies (like `allow_command_registration`) are enforced at load-time by inspecting the plugin's manifest (`plugin.toml`). If a plugin declares a capability that violates the sandbox policy, the debugger will refuse to load it and will emit a `SandboxViolation` error.

Other policies (like `allow_file_write` or `allow_network`) are intended to serve as a runtime contract. Note that because plugins are loaded as native dynamic libraries (not WebAssembly), the debugger cannot perfectly isolate filesystem or network calls at the syscall level without external OS-level sandboxing (like seccomp, AppArmor, or containers). Teams should use these policy flags to declare their operational constraints, and plugin authors should respect these flags when executing.

## Example

See `examples/plugin-policy.toml` for a complete example.