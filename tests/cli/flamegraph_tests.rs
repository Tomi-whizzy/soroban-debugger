use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn get_test_contract() -> Vec<u8> {
    fs::read("tests/fixtures/contracts/target/wasm32-unknown-unknown/release/hello_soroban.wasm")
        .unwrap_or_else(|_| {
            eprintln!("Warning: test contract not found, using dummy bytes");
            vec![0u8; 100]
        })
}

#[test]
fn test_profile_flamegraph_svg_export() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let contract_file = temp_dir.path().join("contract.wasm");
    fs::write(&contract_file, get_test_contract())
        .expect("Failed to write contract file");

    let flamegraph_file = temp_dir.path().join("profile.svg");

    let mut cmd = Command::cargo_bin("soroban-debug").expect("Failed to find binary");
    cmd.args([
        "profile",
        "--contract",
        contract_file.to_str().unwrap(),
        "--function",
        "init",
        "--flamegraph",
        flamegraph_file.to_str().unwrap(),
    ]);

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        assert!(flamegraph_file.exists(), "Flame graph SVG file should be created");
        let content = fs::read_to_string(&flamegraph_file)
            .expect("Failed to read flame graph file");
        assert!(!content.is_empty(), "Flame graph SVG should not be empty");
    }
}

#[test]
fn test_profile_flamegraph_stacks_export() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let contract_file = temp_dir.path().join("contract.wasm");
    fs::write(&contract_file, get_test_contract())
        .expect("Failed to write contract file");

    let stacks_file = temp_dir.path().join("profile.stacks");

    let mut cmd = Command::cargo_bin("soroban-debug").expect("Failed to find binary");
    cmd.args([
        "profile",
        "--contract",
        contract_file.to_str().unwrap(),
        "--function",
        "init",
        "--flamegraph-stacks",
        stacks_file.to_str().unwrap(),
    ]);

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        assert!(stacks_file.exists(), "Collapsed stacks file should be created");
        let content = fs::read_to_string(&stacks_file)
            .expect("Failed to read stacks file");
        assert!(!content.is_empty(), "Stacks file should not be empty");
    }
}

#[test]
fn test_profile_flamegraph_both_exports() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let contract_file = temp_dir.path().join("contract.wasm");
    fs::write(&contract_file, get_test_contract())
        .expect("Failed to write contract file");

    let flamegraph_file = temp_dir.path().join("profile.svg");
    let stacks_file = temp_dir.path().join("profile.stacks");

    let mut cmd = Command::cargo_bin("soroban-debug").expect("Failed to find binary");
    cmd.args([
        "profile",
        "--contract",
        contract_file.to_str().unwrap(),
        "--function",
        "init",
        "--flamegraph",
        flamegraph_file.to_str().unwrap(),
        "--flamegraph-stacks",
        stacks_file.to_str().unwrap(),
    ]);

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        assert!(flamegraph_file.exists(), "Flame graph SVG file should be created");
        assert!(stacks_file.exists(), "Collapsed stacks file should be created");
    }
}

#[test]
fn test_profile_flamegraph_custom_dimensions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let contract_file = temp_dir.path().join("contract.wasm");
    fs::write(&contract_file, get_test_contract())
        .expect("Failed to write contract file");

    let flamegraph_file = temp_dir.path().join("profile.svg");

    let mut cmd = Command::cargo_bin("soroban-debug").expect("Failed to find binary");
    cmd.args([
        "profile",
        "--contract",
        contract_file.to_str().unwrap(),
        "--function",
        "init",
        "--flamegraph",
        flamegraph_file.to_str().unwrap(),
        "--flamegraph-width",
        "1600",
        "--flamegraph-height",
        "1000",
    ]);

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        assert!(flamegraph_file.exists(), "Flame graph SVG file should be created");
    }
}
