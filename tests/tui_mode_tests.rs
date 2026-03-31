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
fn tui_smoke_starts_and_exits_cleanly() {
    let wasm = fixture_wasm("counter");
    if !wasm.exists() {
        eprintln!(
            "Skipping test: fixture not found at {}. Run tests/fixtures/build.sh to build fixtures.",
            wasm.display()
        );
        return;
    }

    let output = base_cmd()
        .env("SOROBAN_DEBUG_TUI_SMOKE", "1")
        .args([
            "tui",
            "--contract",
            wasm.to_str().unwrap(),
            "--function",
            "get",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
