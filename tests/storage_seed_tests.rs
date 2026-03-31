use soroban_debugger::runtime::executor::ContractExecutor;

fn fixture_wasm(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(format!("{name}.wasm"))
}

#[test]
fn storage_seed_changes_execution_and_snapshot() {
    let wasm_path = fixture_wasm("counter");
    if !wasm_path.exists() {
        eprintln!(
            "Skipping test: fixture not found at {}. Run tests/fixtures/build.sh to build fixtures.",
            wasm_path.display()
        );
        return;
    }

    let wasm = std::fs::read(&wasm_path).expect("read fixture wasm");
    let mut executor = ContractExecutor::new(wasm).expect("create executor");

    executor
        .set_initial_storage(r#"{"c": 41}"#.to_string())
        .expect("seed storage");

    let result = executor.execute("get", None).expect("execute get");
    assert!(
        result.contains("I64(41)"),
        "expected seeded value in get result, got: {result}"
    );

    let snapshot = executor.get_storage_snapshot().expect("snapshot");
    assert!(
        snapshot.values().any(|v| v.contains("I64(41)")),
        "expected seeded value in snapshot, got: {snapshot:?}"
    );

    let result2 = executor
        .execute("increment", None)
        .expect("execute increment");
    assert!(
        result2.contains("I64(42)"),
        "expected seeded+1 value, got: {result2}"
    );
}

#[test]
fn storage_seed_rejects_malformed_json() {
    let wasm_path = fixture_wasm("counter");
    if !wasm_path.exists() {
        return;
    }

    let wasm = std::fs::read(&wasm_path).unwrap();
    let mut executor = ContractExecutor::new(wasm).unwrap();
    let err = executor
        .set_initial_storage("{not_json".to_string())
        .unwrap_err()
        .to_string();
    assert!(err.contains("Failed to parse initial storage JSON"));
}
