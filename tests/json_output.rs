use soroban_debugger::cli::output::CommandOutput;

#[test]
fn test_json_output_valid() {
    let output = CommandOutput {
        status: "success".to_string(),
        result: Some("ok"),
        budget: None,
        errors: None,
        hints: None,
    };

    let json = serde_json::to_string(&output).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());
}

#[test]
fn test_json_contains_required_fields() {
    let output = CommandOutput::<()> {
        status: "error".to_string(),
        result: None,
        budget: None,
        errors: Some(vec!["failure".to_string()]),
        hints: None,
    };

    let value = serde_json::to_value(output).unwrap();

    assert!(value.get("status").is_some());
    assert!(value.get("result").is_some());
    assert!(value.get("budget").is_some());
    assert!(value.get("errors").is_some());
}

#[test]
fn test_json_includes_hints_when_present() {
    let output = CommandOutput::<()> {
        status: "error".to_string(),
        result: None,
        budget: None,
        errors: Some(vec!["execution failed".to_string()]),
        hints: Some(vec!["Action: Review logs".to_string()]),
    };

    let value = serde_json::to_value(output).unwrap();
    let hints_array = value.get("hints").expect("hints field should be present");
    assert!(hints_array.is_array());
    assert_eq!(hints_array[0].as_str().unwrap(), "Action: Review logs");
}
