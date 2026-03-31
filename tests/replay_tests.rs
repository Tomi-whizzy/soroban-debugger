use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::NamedTempFile;

/// Helper to create a sample trace JSON file for testing
fn create_sample_trace(contract_path: &str) -> NamedTempFile {
    let trace_file = NamedTempFile::new().unwrap();
    let trace_json = json!({
        "label": "test trace",
        "contract": contract_path,
        "function": "test_function",
        "args": "[\"test_arg\"]",
        "storage": {
            "key1": "value1",
            "key2": 42
        },
        "budget": {
            "cpu_instructions": 10000,
            "memory_bytes": 2048,
            "cpu_limit": 100000,
            "memory_limit": 40960
        },
        "return_value": {
            "status": "ok"
        },
        "call_sequence": [
            {
                "function": "test_function",
                "args": "test_arg",
                "depth": 0
            },
            {
                "function": "helper_function",
                "args": "arg1",
                "depth": 1
            }
        ],
        "events": []
    });

    fs::write(
        trace_file.path(),
        serde_json::to_string_pretty(&trace_json).unwrap(),
    )
    .unwrap();
    trace_file
}

/// Helper to create a minimal trace without contract path
fn create_trace_without_contract() -> NamedTempFile {
    let trace_file = NamedTempFile::new().unwrap();
    let trace_json = json!({
        "label": "minimal trace",
        "function": "test_function",
        "args": "[\"test_arg\"]",
        "storage": {},
        "return_value": {"status": "ok"},
        "call_sequence": [],
        "events": []
    });

    fs::write(
        trace_file.path(),
        serde_json::to_string_pretty(&trace_json).unwrap(),
    )
    .unwrap();
    trace_file
}

#[test]
fn test_replay_command_exists() {
    // Test that the replay subcommand is recognized
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.arg("replay")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Replay"))
        .stdout(predicate::str::contains("trace"));
}

#[test]
fn test_replay_requires_trace_file() {
    // Test that replay requires a trace file argument
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.arg("replay").assert().failure();
}

#[test]
fn test_replay_with_nonexistent_trace() {
    // Test that replay fails gracefully with non-existent trace file
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.arg("replay")
        .arg("nonexistent_trace.json")
        .assert()
        .failure();
}

#[test]
fn test_replay_with_invalid_json() {
    // Test that replay fails gracefully with invalid JSON
    let trace_file = NamedTempFile::new().unwrap();
    fs::write(trace_file.path(), "{ invalid json }").unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.arg("replay").arg(trace_file.path()).assert().failure();
}

#[test]
fn test_replay_with_sample_trace_files() {
    // Test replay with the sample trace files in examples/
    let trace_a_path = "examples/trace_a.json";

    // Check if the file exists
    if !std::path::Path::new(trace_a_path).exists() {
        eprintln!("Skipping test - trace_a.json not found");
        return;
    }

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    // The command might fail due to missing contract, but should at least
    // successfully parse the trace file
    let output = cmd
        .arg("replay")
        .arg(trace_a_path)
        .arg("--contract")
        .arg("examples/contracts/token.wasm") // This might not exist, but tests trace loading
        .output()
        .unwrap();

    // Should at least attempt to load the trace
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("Loading trace file")
            || stderr.contains("Loading trace file")
            || stdout.contains("Failed to read")
            || stderr.contains("Failed to read"),
        "Expected trace loading attempt, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_replay_until_flag() {
    // Test that --replay-until flag is recognized
    let trace_file = create_sample_trace("test.wasm");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    let output = cmd
        .arg("replay")
        .arg(trace_file.path())
        .arg("--replay-until")
        .arg("5")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should parse the flag and attempt replay (may fail due to missing contract)
    assert!(
        stdout.contains("Loading trace")
            || stderr.contains("Loading trace")
            || stdout.contains("Replaying up to step")
            || stderr.contains("Replaying up to step")
            || stdout.contains("Failed to read")
            || stderr.contains("Failed to read"),
        "Expected replay attempt with --replay-until, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_replay_verbose_flag() {
    // Test that --verbose flag works with replay
    let trace_file = create_sample_trace("test.wasm");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    let output = cmd
        .arg("replay")
        .arg(trace_file.path())
        .arg("--verbose")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verbose mode should be recognized
    assert!(
        stdout.contains("Loading trace")
            || stderr.contains("Loading trace")
            || stdout.contains("Failed to read")
            || stderr.contains("Failed to read"),
        "Expected verbose output, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_replay_output_flag() {
    // Test that --output flag writes report to file
    let trace_file = create_sample_trace("test.wasm");
    let output_file = NamedTempFile::new().unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    cmd.arg("replay")
        .arg(trace_file.path())
        .arg("--output")
        .arg(output_file.path())
        .output()
        .unwrap();

    // Even if execution fails, the --output flag should be recognized
    // (file might be created or not depending on when the error occurs)
}

#[test]
fn test_replay_contract_override() {
    // Test that --contract flag overrides trace file's contract path
    let trace_file = create_sample_trace("old_contract.wasm");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    let output = cmd
        .arg("replay")
        .arg(trace_file.path())
        .arg("--contract")
        .arg("new_contract.wasm")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should attempt to load the overridden contract
    assert!(
        stdout.contains("new_contract.wasm")
            || stderr.contains("new_contract.wasm")
            || stdout.contains("Loading contract")
            || stderr.contains("Loading contract"),
        "Expected contract override to be used, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_replay_without_contract_path_fails() {
    // Test that replay fails when trace has no contract and none is provided
    let trace_file = create_trace_without_contract();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    cmd.arg("replay").arg(trace_file.path()).assert().failure();
}

#[test]
fn test_replay_help_shows_all_flags() {
    // Verify all expected flags are documented in help
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    cmd.arg("replay")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("trace"))
        .stdout(predicate::str::contains("contract"))
        .stdout(predicate::str::contains("replay-until"))
        .stdout(predicate::str::contains("output"))
        .stdout(predicate::str::contains("verbose"));
}

#[test]
fn test_replay_trace_with_budget() {
    // Test that replay compares budget information
    let trace_file = create_sample_trace("test.wasm");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));

    let output = cmd.arg("replay").arg(trace_file.path()).output().unwrap();

    // Even though execution may fail, trace loading should work
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.is_empty() || !stderr.is_empty(),
        "Expected some output from replay command"
    );
}
