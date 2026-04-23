# Soroban Debugger Documentation Index

Welcome to the Soroban Debugger documentation. This index helps you navigate the available guides, tutorials, and references.

## 🏁 Getting Started
- [Getting Started Guide](getting-started.md) — Your first steps with the debugger.
- [First Debug Session](tutorials/first-debug.md) — A step-by-step walkthrough.
- [Installation Guide](installation.md) — Detailed installation instructions for all platforms.

## 🛠️ Core Features
- [Source-Level Debugging](source-level-debugging.md) — Debugging Rust source instead of WASM.
- [Instruction Stepping](instruction-stepping.md) — How to use step, next, and finish.
- [Breakpoints Reference](breakpoints.md) — Setting and managing breakpoints.
- [Storage Inspection](storage-snapshot.md) — Viewing and filtering contract storage.

## 🌐 Remote & Advanced Debugging
- [Remote Debugging Guide](remote-debugging.md) — Debugging contracts in CI or on remote hosts.
- [Remote Troubleshooting](remote-troubleshooting.md) — Fixing connection and auth issues.
- [Batch Execution](batch-execution.md) — Running functions with multiple argument sets.
- [Replay Artifacts](replay-artifacts.md) — Capturing and replaying execution traces.

## 📈 Analysis & Security
- [Security Rules Reference](security-rules.md) — List of security rules detected by the analyzer.
- [Optimization Guide](optimization-guide.md) — Tips for reducing gas and resource usage.
- [Resource Timeline](resource-timeline.md) — Understanding budget consumption over time.
- [Upgrade Classes](upgrade-classes.md) — How the debugger classifies contract upgrades.

## 🎓 Tutorials
- [Debugging Auth Errors](tutorials/debug-auth-errors.md) — Diagnosing `require_auth()` failures.
- [Scenario Runner Cookbook](tutorials/scenario-runner.md) — Writing automated integration tests.
- [Symbolic Analysis Budgets](tutorials/symbolic-analysis-budgets.md) — Configuring symbolic exploration.
- [Understanding Budget Trends](tutorials/understanding-budget.md) — Visualizing resource usage.

## 🤝 Contributing & Community
- [Contributing Guide](../CONTRIBUTING.md) — How to help improve the debugger.
- [Code of Conduct](../CODE_OF_CONDUCT.md) — Our community standards.
- [Security Policy](../SECURITY.md) — How to report vulnerabilities.
- [FAQ](faq.md) — Frequently asked questions.

## 📄 Reference
- [CLI Command Index](cli-command-groups.md) — Detailed reference for all CLI subcommands.
- [Trace JSON Schema](trace-schema.md) — Format of exported execution traces.
- [Plugin API](plugin-api.md) — Documentation for the debugger plugin system.
