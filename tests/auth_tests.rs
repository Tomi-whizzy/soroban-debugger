/// Tests for the Authorization Tree Inspector (issue #206).
///
/// Covers:
/// - AuthNode construction and field values
/// - AuthStatus serialization / deserialization
/// - Nested tree display (success and failure scenarios)
/// - JSON output including address, status, sub_invocations
/// - has_failures() propagation through tree
/// - build_failed_nodes() helper
/// - CLI flag acceptance (--show-auth, --json)
use assert_cmd::Command;
use predicates::prelude::*;
use soroban_debugger::inspector::auth::{AuthInspector, AuthNode, AuthStatus};
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    Address, Env, Symbol, Val, Vec as SorobanVec,
};

// ── Helpers ───────────────────────────────────────────────────────────────

fn make_node(function: &str, contract_id: &str, status: AuthStatus) -> AuthNode {
    AuthNode {
        address: "GABC123".to_string(),
        function: function.to_string(),
        contract_id: contract_id.to_string(),
        status,
        sub_invocations: vec![],
    }
}

// ── AuthStatus ────────────────────────────────────────────────────────────

#[test]
fn test_auth_status_as_str() {
    assert_eq!(AuthStatus::Authorized.as_str(), "authorized");
    assert_eq!(AuthStatus::Missing.as_str(), "missing");
    assert_eq!(AuthStatus::Failed.as_str(), "failed");
}

#[test]
fn test_auth_status_serialization() {
    assert_eq!(
        serde_json::to_string(&AuthStatus::Authorized).unwrap(),
        "\"authorized\""
    );
    assert_eq!(
        serde_json::to_string(&AuthStatus::Missing).unwrap(),
        "\"missing\""
    );
    assert_eq!(
        serde_json::to_string(&AuthStatus::Failed).unwrap(),
        "\"failed\""
    );
}

#[test]
fn test_auth_status_deserialization() {
    let s: AuthStatus = serde_json::from_str("\"authorized\"").unwrap();
    assert_eq!(s, AuthStatus::Authorized);

    let s: AuthStatus = serde_json::from_str("\"missing\"").unwrap();
    assert_eq!(s, AuthStatus::Missing);

    let s: AuthStatus = serde_json::from_str("\"failed\"").unwrap();
    assert_eq!(s, AuthStatus::Failed);
}

// ── AuthNode fields ───────────────────────────────────────────────────────

#[test]
fn test_auth_node_fields_populated() {
    let node = make_node("transfer", "CTOKEN", AuthStatus::Authorized);
    assert_eq!(node.function, "transfer");
    assert_eq!(node.contract_id, "CTOKEN");
    assert_eq!(node.address, "GABC123");
    assert_eq!(node.status, AuthStatus::Authorized);
    assert!(node.sub_invocations.is_empty());
}

// ── has_failures() ────────────────────────────────────────────────────────

#[test]
fn test_has_failures_false_when_all_authorized() {
    let node = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "C1".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![make_node("inner", "C2", AuthStatus::Authorized)],
    };
    assert!(!node.has_failures());
}

#[test]
fn test_has_failures_true_for_missing_self() {
    let node = make_node("transfer", "C1", AuthStatus::Missing);
    assert!(node.has_failures());
}

#[test]
fn test_has_failures_true_for_failed_self() {
    let node = make_node("transfer", "C1", AuthStatus::Failed);
    assert!(node.has_failures());
}

#[test]
fn test_has_failures_propagates_from_child() {
    let parent = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "C1".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![make_node("inner", "C2", AuthStatus::Missing)],
    };
    assert!(parent.has_failures());
}

#[test]
fn test_has_failures_deep_nested() {
    let leaf = make_node("deep", "C3", AuthStatus::Failed);
    let mid = AuthNode {
        address: "G1".to_string(),
        function: "mid".to_string(),
        contract_id: "C2".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![leaf],
    };
    let root = AuthNode {
        address: "G1".to_string(),
        function: "root".to_string(),
        contract_id: "C1".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![mid],
    };
    assert!(root.has_failures());
}

// ── to_json ───────────────────────────────────────────────────────────────

#[test]
fn test_to_json_includes_address_and_status() {
    let node = AuthNode {
        address: "GABC123".to_string(),
        function: "transfer".to_string(),
        contract_id: "CTOKEN".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![],
    };
    let json = AuthInspector::to_json(&[node]).unwrap();
    assert!(json.contains("\"address\""), "JSON must have address field");
    assert!(json.contains("GABC123"), "JSON must have address value");
    assert!(json.contains("\"status\""), "JSON must have status field");
    assert!(
        json.contains("\"authorized\""),
        "JSON must have status value"
    );
    assert!(json.contains("\"function\""));
    assert!(json.contains("transfer"));
    assert!(json.contains("\"contract_id\""));
    assert!(json.contains("CTOKEN"));
}

#[test]
fn test_to_json_nested_sub_invocations() {
    let child = AuthNode {
        address: "G2".to_string(),
        function: "inner_fn".to_string(),
        contract_id: "CINNER".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![],
    };
    let parent = AuthNode {
        address: "G1".to_string(),
        function: "outer_fn".to_string(),
        contract_id: "COUTER".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![child],
    };
    let json = AuthInspector::to_json(&[parent]).unwrap();
    assert!(json.contains("outer_fn"));
    assert!(json.contains("inner_fn"));
    assert!(json.contains("CINNER"));
    assert!(json.contains("sub_invocations"));
}

#[test]
fn test_to_json_missing_status_in_output() {
    let node = make_node("broken", "CBAD", AuthStatus::Missing);
    let json = AuthInspector::to_json(&[node]).unwrap();
    assert!(
        json.contains("\"missing\""),
        "Missing auth node must appear in JSON with 'missing' status"
    );
}

#[test]
fn test_to_json_failed_status_in_output() {
    let node = make_node("broken", "CBAD", AuthStatus::Failed);
    let json = AuthInspector::to_json(&[node]).unwrap();
    assert!(
        json.contains("\"failed\""),
        "Failed auth node must appear in JSON with 'failed' status"
    );
}

#[test]
fn test_to_json_empty_tree() {
    let json = AuthInspector::to_json(&[]).unwrap();
    assert_eq!(json.trim(), "[]");
}

#[test]
fn test_to_json_value_is_array() {
    let node = make_node("fn", "C1", AuthStatus::Authorized);
    let val = AuthInspector::to_json_value(&[node]);
    assert!(val.is_array());
    assert_eq!(val.as_array().unwrap().len(), 1);
}

#[test]
fn test_to_json_roundtrip() {
    let node = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "CTOKEN".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![make_node("inner", "C2", AuthStatus::Missing)],
    };
    let json = AuthInspector::to_json(&[node]).unwrap();
    let parsed: Vec<AuthNode> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].function, "transfer");
    assert_eq!(parsed[0].sub_invocations[0].status, AuthStatus::Missing);
}

// ── build_failed_nodes ────────────────────────────────────────────────────

#[test]
fn test_build_failed_nodes_creates_missing_nodes() {
    let required = vec![("GABC", "CTOKEN", "transfer")];
    let nodes = AuthInspector::build_failed_nodes(&required);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].status, AuthStatus::Missing);
    assert_eq!(nodes[0].function, "transfer");
    assert_eq!(nodes[0].contract_id, "CTOKEN");
    assert_eq!(nodes[0].address, "GABC");
}

#[test]
fn test_build_failed_nodes_multiple() {
    let required = vec![("G1", "C1", "fn_a"), ("G2", "C2", "fn_b")];
    let nodes = AuthInspector::build_failed_nodes(&required);
    assert_eq!(nodes.len(), 2);
    assert!(nodes.iter().all(|n| n.status == AuthStatus::Missing));
}

// ── display: no panics ────────────────────────────────────────────────────

#[test]
fn test_display_empty_tree_no_panic() {
    AuthInspector::display(&[]);
}

#[test]
fn test_display_single_authorized_node_no_panic() {
    let node = make_node("transfer", "CTOKEN", AuthStatus::Authorized);
    AuthInspector::display(&[node]);
}

#[test]
fn test_display_single_failed_node_no_panic() {
    let node = make_node("transfer", "CTOKEN", AuthStatus::Missing);
    AuthInspector::display(&[node]);
}

#[test]
fn test_display_with_summary_all_pass_no_panic() {
    let node = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "C1".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![make_node("inner", "C2", AuthStatus::Authorized)],
    };
    AuthInspector::display_with_summary(&[node]);
}

#[test]
fn test_display_with_summary_with_failure_no_panic() {
    let node = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "C1".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![make_node("inner", "C2", AuthStatus::Missing)],
    };
    AuthInspector::display_with_summary(&[node]);
}

#[test]
fn test_display_nested_three_levels_no_panic() {
    let leaf = make_node("leaf_fn", "CLEAF", AuthStatus::Failed);
    let mid = AuthNode {
        address: "G2".to_string(),
        function: "mid_fn".to_string(),
        contract_id: "CMID".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![leaf],
    };
    let root = AuthNode {
        address: "G1".to_string(),
        function: "root_fn".to_string(),
        contract_id: "CROOT".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![mid],
    };
    AuthInspector::display_with_summary(&[root]);
}

// ── get_auth_tree from env ────────────────────────────────────────────────

#[test]
fn test_get_auth_tree_empty_env_returns_empty() {
    let env = Env::default();
    let tree = AuthInspector::get_auth_tree(&env).unwrap();
    assert!(tree.is_empty(), "Empty env should produce empty auth tree");
}

#[test]
fn test_auth_node_serialization_legacy_compat() {
    // Ensure old fields (function, contract_id, sub_invocations) are still present.
    let node = AuthNode {
        address: "GABC".to_string(),
        function: "transfer".to_string(),
        contract_id: "C123".to_string(),
        status: AuthStatus::Authorized,
        sub_invocations: vec![AuthNode {
            address: "GDEF".to_string(),
            function: "inner".to_string(),
            contract_id: "C456".to_string(),
            status: AuthStatus::Authorized,
            sub_invocations: vec![],
        }],
    };

    let json = AuthInspector::to_json(&[node]).unwrap();
    assert!(json.contains("transfer"));
    assert!(json.contains("inner"));
    assert!(json.contains("C123"));
    assert!(json.contains("C456"));
}

#[test]
fn test_auth_inspector_node_from_sdk_types() {
    let env = Env::default();
    let contract_id = Address::generate(&env);
    let function_name = Symbol::new(&env, "test_func");
    let args = SorobanVec::<Val>::new(&env);

    let _invocation = AuthorizedInvocation {
        function: AuthorizedFunction::Contract((
            contract_id.clone(),
            function_name.clone(),
            args.clone(),
        )),
        sub_invocations: std::vec::Vec::new(),
    };

    // Build equivalent node manually and verify display + JSON work.
    let nodes = vec![AuthNode {
        address: format!("{:?}", contract_id),
        function: format!("{:?}({:?})", function_name, args),
        contract_id: format!("{:?}", contract_id),
        status: AuthStatus::Authorized,
        sub_invocations: vec![],
    }];

    AuthInspector::display(&nodes);
    let json = AuthInspector::to_json(&nodes).unwrap();
    assert!(json.contains("test_func"));
    assert!(json.contains("\"authorized\""));
}

// ── CLI integration ───────────────────────────────────────────────────────

#[test]
fn test_run_command_auth_flags() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_soroban-debug"));
    cmd.arg("run").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--show-auth"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn test_show_auth_flag_accepted_by_parser() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let wasm = dir.path().join("c.wasm");
    std::fs::write(&wasm, b"dummy").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_soroban-debug"))
        .args([
            "run",
            "--contract",
            wasm.to_str().unwrap(),
            "--function",
            "test",
            "--show-auth",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized"),
        "--show-auth should be recognised: {stderr}"
    );
}

#[test]
fn test_show_auth_with_json_flag_accepted() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let wasm = dir.path().join("c.wasm");
    std::fs::write(&wasm, b"dummy").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_soroban-debug"))
        .args([
            "run",
            "--contract",
            wasm.to_str().unwrap(),
            "--function",
            "test",
            "--show-auth",
            "--json",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized"),
        "--show-auth --json should be recognised: {stderr}"
    );
}
