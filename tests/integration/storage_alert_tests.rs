//! Integration tests for storage change alerts

use soroban_debugger::inspector::storage::{StorageInspector};
use std::collections::HashMap;

#[test]
fn test_alert_on_exact_key() {
    let before = HashMap::from([
        ("balance:alice".to_string(), "1000".to_string()),
        ("balance:bob".to_string(), "500".to_string()),
    ]);
    let after = HashMap::from([
        ("balance:alice".to_string(), "2000".to_string()),
        ("balance:bob".to_string(), "500".to_string()),
    ]);
    let alerts = vec!["balance:alice".to_string()];
    let diff = StorageInspector::compute_diff(&before, &after, &alerts);
    assert_eq!(diff.triggered_alerts, vec!["balance:alice".to_string()]);
}

#[test]
fn test_alert_on_wildcard_pattern() {
    let before = HashMap::from([
        ("balance:alice".to_string(), "1000".to_string()),
        ("balance:bob".to_string(), "500".to_string()),
        ("total_supply".to_string(), "1500".to_string()),
    ]);
    let after = HashMap::from([
        ("balance:alice".to_string(), "2000".to_string()),
        ("balance:bob".to_string(), "700".to_string()),
        ("total_supply".to_string(), "2700".to_string()),
    ]);
    let alerts = vec!["balance:*".to_string()];
    let mut diff = StorageInspector::compute_diff(&before, &after, &alerts);
    diff.triggered_alerts.sort();
    assert_eq!(diff.triggered_alerts, vec!["balance:alice", "balance:bob"]);
}

#[test]
fn test_alert_on_deleted_key() {
    let before = HashMap::from([
        ("critical_key".to_string(), "secret".to_string()),
        ("other_key".to_string(), "value".to_string()),
    ]);
    let after = HashMap::from([
        ("other_key".to_string(), "value".to_string()),
    ]);
    let alerts = vec!["critical_key".to_string()];
    let diff = StorageInspector::compute_diff(&before, &after, &alerts);
    assert_eq!(diff.triggered_alerts, vec!["critical_key".to_string()]);
}

#[test]
fn test_alert_on_regex_pattern() {
    let before = HashMap::from([
        ("user_1".to_string(), "alice".to_string()),
        ("user_2".to_string(), "bob".to_string()),
        ("admin".to_string(), "root".to_string()),
    ]);
    let after = HashMap::from([
        ("user_1".to_string(), "alice2".to_string()),
        ("user_2".to_string(), "bob".to_string()),
        ("admin".to_string(), "root".to_string()),
    ]);
    let alerts = vec![r"re:^user_\\d+$".to_string()];
    let diff = StorageInspector::compute_diff(&before, &after, &alerts);
    assert_eq!(diff.triggered_alerts, vec!["user_1".to_string()]);
}
