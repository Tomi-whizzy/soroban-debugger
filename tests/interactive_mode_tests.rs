use assert_cmd::Command;

fn fixture_wasm(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(format!("{name}.wasm"))
}

fn base_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.env("NO_COLOR", "1");
    cmd.env("NO_BANNER", "1");
    cmd
}

#[test]
fn interactive_accepts_basic_commands_and_exits() {
    let wasm = fixture_wasm("counter");
    if !wasm.exists() {
        eprintln!(
            "Skipping test: fixture not found at {}. Run tests/fixtures/build.sh to build fixtures.",
            wasm.display()
        );
        return;
    }

    let output = base_cmd()
        .args([
            "interactive",
            "--contract",
            wasm.to_str().unwrap(),
            "--function",
            "get",
        ])
        .write_stdin("inspect\ncontinue\nquit\n")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
