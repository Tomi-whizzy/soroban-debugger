# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Added `invocation_reason` to `DynamicTraceEvent` protocol to support better cross-contract call analysis.
- New `format_resource_timeline` utility for generating markdown reports.
- Added `CODE_OF_CONDUCT.md` and `SECURITY.md`.
- Added support for connection timeouts and retries in remote debugging.

### Fixed
- Fixed protocol breakage in security analyzer tests.
- Resolved man page drift by regenerating documentation.
- Improved error messaging for authorization failures.
- Fixed `SecurityAnalyzer::analyze` signature consistency across tests and examples.

### Changed
- Standardized project landing page links in README.
- Updated FAQ with more detailed troubleshooting for contract panics.

## [0.1.0] - 2024-04-20

### Added
- Initial release of the Soroban Debugger.
- Support for interactive debugging with breakpoints and stepping.
- Source-level mapping for Rust contracts.
- Remote debugging server and client.
- Security analyzer with rule-based finding detection.
- Budget and resource tracking.
- Scenario runner for automated integration testing.
