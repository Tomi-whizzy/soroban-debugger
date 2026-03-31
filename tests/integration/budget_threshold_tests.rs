use soroban_debugger::inspector::budget::{BudgetInfo, BudgetInspector, Severity};

#[test]
fn test_threshold_none_below_70() {
    let info = BudgetInfo {
        cpu_instructions: 69,
        cpu_limit: 100,
        memory_bytes: 60,
        memory_limit: 100,
    };
    let warnings = BudgetInspector::check_thresholds(&info);
    assert!(warnings.is_empty(), "Expected no warnings for 69% usage");
}

#[test]
fn test_threshold_yellow_at_70() {
    let info = BudgetInfo {
        cpu_instructions: 70,
        cpu_limit: 100,
        memory_bytes: 0,
        memory_limit: 100,
    };
    let warnings = BudgetInspector::check_thresholds(&info);
    assert_eq!(warnings.len(), 1);
    assert!(matches!(warnings[0].severity, Severity::Yellow));
}

#[test]
fn test_threshold_red_at_85() {
    let info = BudgetInfo {
        cpu_instructions: 85,
        cpu_limit: 100,
        memory_bytes: 0,
        memory_limit: 100,
    };
    let warnings = BudgetInspector::check_thresholds(&info);
    assert_eq!(warnings.len(), 1);
    assert!(matches!(warnings[0].severity, Severity::Red));
    assert!(
        warnings[0].suggestion.is_none(),
        "85% should not have suggestions"
    );
}

#[test]
fn test_threshold_critical_at_90() {
    let info = BudgetInfo {
        cpu_instructions: 90,
        cpu_limit: 100,
        memory_bytes: 0,
        memory_limit: 100,
    };
    let warnings = BudgetInspector::check_thresholds(&info);
    assert_eq!(warnings.len(), 1);
    assert!(matches!(warnings[0].severity, Severity::Critical));
    assert!(
        warnings[0].suggestion.is_some(),
        "90% MUST have optimization suggestions"
    );

    let suggestion = warnings[0].suggestion.as_ref().unwrap();
    assert!(
        suggestion.contains("optimizing"),
        "Suggestion should mention optimization"
    );
}

#[test]
fn test_multiple_critical_warnings() {
    let info = BudgetInfo {
        cpu_instructions: 95,
        cpu_limit: 100,
        memory_bytes: 92,
        memory_limit: 100,
    };
    let warnings = BudgetInspector::check_thresholds(&info);
    assert_eq!(
        warnings.len(),
        2,
        "Expected warnings for both CPU and Memory"
    );
    assert!(matches!(warnings[0].severity, Severity::Critical));
    assert!(matches!(warnings[1].severity, Severity::Critical));
}
