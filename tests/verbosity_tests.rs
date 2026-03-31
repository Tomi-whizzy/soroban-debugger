/// Tests for --quiet and --verbose output flags (issue #187).
///
/// These tests verify:
/// - `--quiet` suppresses informational output (only errors and return value shown)
/// - `--verbose` shows all internal details
/// - Both flags are accepted globally across subcommands
/// - Normal mode (default) shows standard informational output
use soroban_debugger::ui::formatter::Formatter;

// ── Unit tests for Formatter verbosity helpers ─────────────────────────────

#[test]
fn test_formatter_default_verbosity_is_normal() {
    // After reset to Normal (1), neither quiet nor verbose should be true.
    Formatter::set_verbosity(1);
    assert!(!Formatter::is_quiet(), "Normal mode should not be quiet");
    assert!(
        !Formatter::is_verbose(),
        "Normal mode should not be verbose"
    );
}

#[test]
fn test_formatter_quiet_mode() {
    Formatter::set_verbosity(0);
    assert!(
        Formatter::is_quiet(),
        "Quiet mode: is_quiet() should be true"
    );
    assert!(
        !Formatter::is_verbose(),
        "Quiet mode: is_verbose() should be false"
    );
    // Reset
    Formatter::set_verbosity(1);
}

#[test]
fn test_formatter_verbose_mode() {
    Formatter::set_verbosity(2);
    assert!(
        !Formatter::is_quiet(),
        "Verbose mode: is_quiet() should be false"
    );
    assert!(
        Formatter::is_verbose(),
        "Verbose mode: is_verbose() should be true"
    );
    // Reset
    Formatter::set_verbosity(1);
}

#[test]
fn test_formatter_verbosity_transitions() {
    // Cycle through all three levels and verify state at each transition.
    for (level, expect_quiet, expect_verbose) in
        [(0u8, true, false), (1, false, false), (2, false, true)]
    {
        Formatter::set_verbosity(level);
        assert_eq!(
            Formatter::is_quiet(),
            expect_quiet,
            "Level {level}: is_quiet mismatch"
        );
        assert_eq!(
            Formatter::is_verbose(),
            expect_verbose,
            "Level {level}: is_verbose mismatch"
        );
    }
    // Reset
    Formatter::set_verbosity(1);
}

// ── CLI integration tests (argument parsing) ───────────────────────────────

#[cfg(test)]
mod cli_flag_tests {
    use assert_cmd::cargo::cargo_bin_cmd;
    use predicates::prelude::*;
    use tempfile::TempDir;

    fn dummy_wasm(dir: &TempDir) -> std::path::PathBuf {
        let p = dir.path().join("contract.wasm");
        std::fs::write(&p, b"dummy").unwrap();
        p
    }

    // ── Flag acceptance ────────────────────────────────────────────────────

    /// --quiet is accepted as a global flag before the subcommand.
    #[test]
    fn test_quiet_flag_accepted_globally() {
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        cmd.args(["--quiet", "--help"]).assert().success();
    }

    /// --verbose is accepted as a global flag before the subcommand.
    #[test]
    fn test_verbose_flag_accepted_globally() {
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        cmd.args(["--verbose", "--help"]).assert().success();
    }

    /// --quiet is accepted before `run` subcommand.
    #[test]
    fn test_quiet_accepted_before_run_subcommand() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        // Arg parse should succeed; execution will fail (invalid WASM) which is fine.
        let output = cmd
            .args([
                "--quiet",
                "run",
                "--contract",
                wasm.to_str().unwrap(),
                "--function",
                "test",
            ])
            .output()
            .unwrap();
        // Must not fail with "unrecognized argument" or similar parse error.
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unrecognized"),
            "Flag should be recognised: {stderr}"
        );
        assert!(
            !stderr.contains("unexpected argument"),
            "Flag should be recognised: {stderr}"
        );
    }

    /// --verbose is accepted before `run` subcommand.
    #[test]
    fn test_verbose_accepted_before_run_subcommand() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        let output = cmd
            .args([
                "--verbose",
                "run",
                "--contract",
                wasm.to_str().unwrap(),
                "--function",
                "test",
            ])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unrecognized"),
            "Flag should be recognised: {stderr}"
        );
    }

    /// --quiet is accepted before `inspect` subcommand.
    #[test]
    fn test_quiet_accepted_before_inspect_subcommand() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        let output = cmd
            .args(["--quiet", "inspect", "--contract", wasm.to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unrecognized"),
            "Flag should be recognised: {stderr}"
        );
    }

    /// --verbose is accepted before `inspect` subcommand.
    #[test]
    fn test_verbose_accepted_before_inspect_subcommand() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        let output = cmd
            .args(["--verbose", "inspect", "--contract", wasm.to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unrecognized"),
            "Flag should be recognised: {stderr}"
        );
    }

    /// --quiet is accepted before `analyze` subcommand.
    #[test]
    fn test_quiet_accepted_before_analyze_subcommand() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        let output = cmd
            .args(["--quiet", "analyze", "--contract", wasm.to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unrecognized"),
            "Flag should be recognised: {stderr}"
        );
    }

    /// --quiet and --verbose are mutually exclusive via flag semantics; supplying both should
    /// let the parser accept them (clap does not make them exclusive by default, the code
    /// resolves priority: quiet wins via `if self.quiet`).
    #[test]
    fn test_quiet_takes_priority_over_verbose_in_help() {
        let mut cmd = cargo_bin_cmd!("soroban-debug");
        // Both flags + --help: should succeed (help is always shown).
        cmd.args(["--quiet", "--verbose", "--help"])
            .assert()
            .success();
    }

    // ── Output suppression in quiet mode ───────────────────────────────────

    /// In --quiet mode the "Loading contract" informational line must NOT appear in stdout.
    /// (We use an invalid WASM so execution fails quickly — we only check stdout content.)
    #[test]
    fn test_quiet_suppresses_loading_info_in_run() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let output = cargo_bin_cmd!("soroban-debug")
            .args([
                "--quiet",
                "run",
                "--contract",
                wasm.to_str().unwrap(),
                "--function",
                "test",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("Loading contract"),
            "Quiet mode: stdout should not contain 'Loading contract', got: {stdout}"
        );
        assert!(
            !stdout.contains("Starting debugger"),
            "Quiet mode: stdout should not contain 'Starting debugger', got: {stdout}"
        );
    }

    /// In --quiet mode the "Inspecting contract" informational line must NOT appear.
    #[test]
    fn test_quiet_suppresses_loading_info_in_inspect() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let output = cargo_bin_cmd!("soroban-debug")
            .args(["--quiet", "inspect", "--contract", wasm.to_str().unwrap()])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("Inspecting contract"),
            "Quiet mode: stdout should not contain 'Inspecting contract', got: {stdout}"
        );
    }

    // ── Help output shows both flags ───────────────────────────────────────

    /// The top-level help must document --quiet.
    #[test]
    fn test_help_documents_quiet_flag() {
        cargo_bin_cmd!("soroban-debug")
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("quiet"));
    }

    /// The top-level help must document --verbose.
    #[test]
    fn test_help_documents_verbose_flag() {
        cargo_bin_cmd!("soroban-debug")
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("verbose"));
    }

    /// `run --help` must mention --verbose (it has its own local --verbose too).
    #[test]
    fn test_run_help_documents_verbose_flag() {
        cargo_bin_cmd!("soroban-debug")
            .args(["run", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("verbose"));
    }

    // ── Normal mode is the default ─────────────────────────────────────────

    /// Without any verbosity flag the binary uses Normal mode — informational
    /// text is printed (e.g. "Loading contract").
    #[test]
    fn test_normal_mode_is_default_shows_info() {
        let dir = TempDir::new().unwrap();
        let wasm = dummy_wasm(&dir);
        let output = cargo_bin_cmd!("soroban-debug")
            .args([
                "run",
                "--contract",
                wasm.to_str().unwrap(),
                "--function",
                "test",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Normal mode should print the "Loading contract" banner.
        assert!(
            stdout.contains("Loading contract"),
            "Normal mode: stdout should contain 'Loading contract', got: {stdout}"
        );
    }
}
