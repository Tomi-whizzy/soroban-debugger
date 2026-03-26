use assert_cmd::Command;
use jsonschema::JSONSchema;
use serde_json::Value;
use std::fs;

fn compile_schema(path: &str) -> JSONSchema {
    let schema_content = fs::read_to_string(path).expect("Failed to read schema file");
    let schema_json: Value =
        serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");
    JSONSchema::compile(&schema_json).expect("Failed to compile schema")
}

fn parse_json_stdout(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("Stdout is not valid UTF-8");
    serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Failed to parse JSON output: {}", stdout))
}

fn assert_schema_valid(schema: &JSONSchema, json_val: &Value, context: &str) {
    match schema.validate(json_val) {
        Ok(()) => {}
        Err(errors) => {
            let details = errors.map(|e| e.to_string()).collect::<Vec<_>>().join("\n");
            panic!("{} schema validation failed:\n{}", context, details);
        }
    }
}

#[test]
fn run_json_output_matches_versioned_schema() {
    let wasm_path = "tests/fixtures/wasm/counter.wasm";
    #[allow(deprecated)]
    let output = Command::cargo_bin("soroban-debug")
        .unwrap()
        .arg("--quiet")
        .arg("run")
        .arg("--contract")
        .arg(wasm_path)
        .arg("--function")
        .arg("increment")
        .arg("--output")
        .arg("json")
        .arg("--show-events")
        .output()
        .expect("Failed to execute run command");

    let json_val = parse_json_stdout(output);
    let schema = compile_schema("tests/schemas/execution_output.json");

    assert_schema_valid(&schema, &json_val, "Run JSON");
}

#[test]
fn analyze_json_output_matches_versioned_schema() {
    let wasm_path = "tests/fixtures/wasm/counter.wasm";
    #[allow(deprecated)]
    let output = Command::cargo_bin("soroban-debug")
        .unwrap()
        .arg("--quiet")
        .arg("analyze")
        .arg("--contract")
        .arg(wasm_path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute analyze command");

    let json_val = parse_json_stdout(output);
    let schema = compile_schema("tests/schemas/analyze_output.json");

    assert_schema_valid(&schema, &json_val, "Analyze JSON");
}

#[test]
fn inspect_json_output_matches_versioned_schema() {
    let wasm_path = "tests/fixtures/wasm/counter.wasm";
    #[allow(deprecated)]
    let output = Command::cargo_bin("soroban-debug")
        .unwrap()
        .arg("--quiet")
        .arg("inspect")
        .arg("--contract")
        .arg(wasm_path)
        .arg("--format")
        .arg("json")
        .arg("--functions")
        .output()
        .expect("Failed to execute inspect command");

    let json_val = parse_json_stdout(output);
    let schema = compile_schema("tests/schemas/inspect_output.json");

    assert_schema_valid(&schema, &json_val, "Inspect JSON");
}

#[test]
fn upgrade_check_json_output_matches_versioned_schema() {
    let wasm_path = "tests/fixtures/wasm/counter.wasm";
    #[allow(deprecated)]
    let output = Command::cargo_bin("soroban-debug")
        .unwrap()
        .arg("--quiet")
        .arg("upgrade-check")
        .arg("--old")
        .arg(wasm_path)
        .arg("--new")
        .arg(wasm_path)
        .arg("--output")
        .arg("json")
        .output()
        .expect("Failed to execute upgrade-check command");

    let json_val = parse_json_stdout(output);
    let schema = compile_schema("tests/schemas/upgrade_check_output.json");

    assert_schema_valid(&schema, &json_val, "Upgrade-check JSON");
}

#[test]
fn schema_rejects_missing_schema_version() {
    let schema = compile_schema("tests/schemas/execution_output.json");
    let invalid = serde_json::json!({
        "command": "run",
        "status": "success",
        "result": {},
        "error": null
    });

    let result = schema.validate(&invalid);
    assert!(
        result.is_err(),
        "schema should reject missing schema_version"
    );
}

#[test]
fn schema_rejects_invalid_envelope_structure() {
    let schema = compile_schema("tests/schemas/analyze_output.json");
    let invalid = serde_json::json!({
        "schema_version": "1.0.0",
        "command": "analyze",
        "status": "ok",
        "payload": {}
    });

    let result = schema.validate(&invalid);
    assert!(
        result.is_err(),
        "schema should reject invalid envelope fields"
    );
}
