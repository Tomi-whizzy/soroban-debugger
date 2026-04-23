# Security Policy

## Supported Versions

The following versions of `soroban-debugger` are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| v0.1.x  | :white_check_mark: |
| < v0.1  | :x:                |

## Reporting a Vulnerability

We take the security of this project seriously. If you believe you have found a security vulnerability, please report it to us responsibly.

**Please do not report security vulnerabilities via public GitHub issues.**

Instead, please send an email to [INSERT SECURITY EMAIL] with a description of the issue and steps to reproduce it. We will acknowledge your report within 48 hours and provide a timeline for a fix.

### What to Include in a Report

- A detailed description of the vulnerability.
- Steps to reproduce the issue (PoC code or a trace file is highly encouraged).
- The potential impact of the vulnerability.
- Any suggested mitigations if known.

### Disclosure Policy

We follow a coordinated disclosure policy. We ask that you give us reasonable time to investigate and resolve the issue before making any information public. In return, we will:

- Respond promptly to your report.
- Keep you informed of our progress.
- Credit you for the discovery in our `CHANGELOG.md` (unless you prefer to remain anonymous).

## Security Philosophy

`soroban-debugger` is a tool for developers to analyze and debug contracts. While we strive for correctness, the tool itself runs in a developer's environment and interacts with potentially untrusted WASM code. We employ several layers of protection:

1. **Host Isolation**: The debugger uses `soroban-env-host` to execute contracts, which provides the primary sandbox.
2. **Resource Limits**: We enforce CPU and memory budgets during execution.
3. **Input Validation**: All CLI arguments and trace files are validated before processing.

Thank you for helping keep the Soroban ecosystem secure!
