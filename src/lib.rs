#![recursion_limit = "256"]

pub mod analyzer;
pub mod batch;
pub mod benchmarks;
pub mod cli;
pub mod client;
pub mod codegen;
pub mod compare;
pub mod config;
pub mod debugger;
pub mod history;
pub mod inspector;
pub mod logging;
pub mod output;
pub mod plugin;
pub mod profiler;
pub mod protocol;
pub mod repeat;
pub mod repl;
pub mod runtime;
pub mod scenario;
pub mod server;
pub mod simulator;
pub mod ui;
pub mod utils;


use miette::Diagnostic;

pub use debugger::engine::DebuggerEngine;
pub use runtime::executor::ContractExecutor;

/// Result type alias for the debugger
pub type Result<T> = miette::Result<T>;

/// Error types for the debugger
#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum DebuggerError {
    #[error("Failed to load WASM file: {0}")]
    #[diagnostic(
        code(debugger::wasm_load_failed),
        help("Action: Check the file path and verify it is a valid compiled Soroban WASM contract. Try rebuilding with `cargo contract build`.\nContext: The debugger requires a valid, readable .wasm file.")
    )]
    WasmLoadError(String),

    #[error("Failed to execute contract: {0}")]
    #[diagnostic(
        code(debugger::execution_failed),
        help("Action: Review the contract logs and failure messages. Re-run with `RUST_BACKTRACE=1` to see host panics and check arguments.\nContext: The execution environment trapped or aborted execution unexpectedly.")
    )]
    ExecutionError(String),

    #[error("Invalid function name: {0}")]
    #[diagnostic(
        code(debugger::invalid_function),
        help("Action: Ensure the function name is spelled exactly as exported by the contract.\nContext: You can use `soroban-debug inspect --functions` to see the list of exported functions.")
    )]
    InvalidFunction(String),

    #[error("Invalid arguments: {0}")]
    #[diagnostic(
        code(debugger::invalid_arguments),
        help("Action: Ensure arguments are passed as a valid JSON array format, e.g., `--args '[\"Alice\", 1000]'.\nContext: Arguments must match the contract signature and correctly serialize.")
    )]
    InvalidArguments(String),

    #[error("Breakpoint error: {0}")]
    #[diagnostic(
        code(debugger::breakpoint_error),
        help("Action: Verify the target function exists and the breakpoint condition syntax is correct.\nContext: Breakpoints fail to apply if the underlying symbol isn't reachable.")
    )]
    BreakpointError(String),

    #[error("Storage error: {0}")]
    #[diagnostic(
        code(debugger::storage_error),
        help("Action: Ensure the snapshot file exists, contains valid JSON, and matches the ledger entry schema.\nContext: A malformed snapshot prevents the ledger state from initializing.")
    )]
    StorageError(String),

    #[error("WASM checksum mismatch.\n  Expected : {expected}\n  Computed : {actual}")]
    #[diagnostic(
        code(debugger::checksum_mismatch),
        help("Action: If you recompiled the contract, supply its new hash or run without the hash verification flag.\nContext: The provided file hash does not match expected remote or snapshot hash.")
    )]
    ChecksumMismatch { expected: String, actual: String },

    #[error("File operation failed: {0}")]
    #[diagnostic(
        code(debugger::file_error),
        help("Action: Check file permissions, path spelling, and directory access rules.\nContext: The tool could not read or write the designated filesystem path.")
    )]
    FileError(String),

    #[error("Network/transport error: {0}")]
    #[diagnostic(
        code(debugger::network_error),
        help("Action: Ensure the remote debug server is online, address is correct, and network firewall permits the connection.\nContext: The transport connection failed to establish or dropped unexpectedly.")
    )]
    NetworkError(String),

    #[error("Request timed out: {operation} (timeout={timeout_ms}ms)")]
    #[diagnostic(
        code(debugger::request_timeout),
        help("Action: Increase the timeout/retry settings if your host environment is slow or congested.\nContext: Network instability or an overloaded host aborted the request.")
    )]
    RequestTimeout { operation: String, timeout_ms: u64 },

    #[error("Authentication failed: {0}")]
    #[diagnostic(
        code(debugger::auth_failed),
        help("Action: Ensure the shared security token matches the server, and the transport protocol is correct.\nContext: The server rejected communication because authentication wasn't verified.")
    )]
    AuthenticationFailed(String),
}
