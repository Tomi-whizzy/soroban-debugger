use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single breakpoint with optional conditions and logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    /// Client-visible breakpoint id.
    pub id: String,
    /// Function name where the breakpoint is set
    pub function: String,
    /// Optional condition expression (e.g., "balance > 1000")
    pub condition: Option<String>,
    /// Optional hit condition (e.g., ">5", "==3", "%2==0")
    pub hit_condition: Option<String>,
    /// Optional log message with variable interpolation (e.g., "Balance: {balance}")
    pub log_message: Option<String>,
    /// Number of times this breakpoint has been hit
    pub hit_count: usize,
}

impl Breakpoint {
    /// Create a simple breakpoint without conditions
    pub fn simple(function: String) -> Self {
        Self {
            id: function.clone(),
            function,
            condition: None,
            hit_condition: None,
            log_message: None,
            hit_count: 0,
        }
    }

    /// Create a breakpoint with a condition
    pub fn with_condition(function: String, condition: String) -> Self {
        Self {
            id: function.clone(),
            function,
            condition: Some(condition),
            hit_condition: None,
            log_message: None,
            hit_count: 0,
        }
    }

    /// Create a breakpoint with a hit condition
    pub fn with_hit_condition(function: String, hit_condition: String) -> Self {
        Self {
            id: function.clone(),
            function,
            condition: None,
            hit_condition: Some(hit_condition),
            log_message: None,
            hit_count: 0,
        }
    }

    /// Create a log point (breakpoint that doesn't pause, just logs)
    pub fn log_point(function: String, log_message: String) -> Self {
        Self {
            id: function.clone(),
            function,
            condition: None,
            hit_condition: None,
            log_message: Some(log_message),
            hit_count: 0,
        }
    }

    /// Increment the hit count
    pub fn increment_hit(&mut self) {
        self.hit_count += 1;
    }

    /// Check if this is a log point (has log message but should not pause)
    pub fn is_log_point(&self) -> bool {
        self.log_message.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct BreakpointSpec {
    pub id: String,
    pub function: String,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BreakpointHit {
    pub should_pause: bool,
    pub log_messages: Vec<String>,
    pub pause_reason: Option<String>,
}

/// Manages breakpoints during debugging
pub struct BreakpointManager {
    breakpoints: HashMap<String, Breakpoint>,
    breakpoint_ids: HashMap<String, String>,
}

impl BreakpointManager {
    /// Create a new breakpoint manager
    pub fn new() -> Self {
        Self {
            breakpoints: HashMap::new(),
            breakpoint_ids: HashMap::new(),
        }
    }

    /// Add or update a breakpoint
    pub fn set(&mut self, breakpoint: Breakpoint) {
        let function = breakpoint.function.clone();
        let id = breakpoint.id.clone();

        if let Some(existing) = self.breakpoints.get(&function) {
            self.breakpoint_ids.remove(&existing.id);
        }

        if let Some(previous_function) = self.breakpoint_ids.get(&id).cloned() {
            self.breakpoints.remove(&previous_function);
        }

        self.breakpoint_ids.insert(id, function.clone());
        self.breakpoints.insert(function, breakpoint);
    }

    /// Add a simple breakpoint at a function name (backward compatibility)
    pub fn add(&mut self, function: &str) {
        self.set(Breakpoint::simple(function.to_string()));
    }

    pub fn add_simple(&mut self, function: &str) {
        self.add(function);
    }

    pub fn add_spec(&mut self, spec: BreakpointSpec) {
        self.set(Breakpoint {
            id: spec.id,
            function: spec.function,
            condition: spec.condition,
            hit_condition: spec.hit_condition,
            log_message: spec.log_message,
            hit_count: 0,
        });
    }

    /// Remove a breakpoint
    pub fn remove(&mut self, function: &str) -> bool {
        self.remove_breakpoint(function).is_some()
    }

    fn remove_breakpoint(&mut self, function: &str) -> Option<Breakpoint> {
        let removed = self.breakpoints.remove(function)?;
        self.breakpoint_ids.remove(&removed.id);
        Some(removed)
    }

    pub fn remove_function(&mut self, function: &str) -> bool {
        self.remove(function)
    }

    pub fn remove_by_id(&mut self, id: &str) -> bool {
        if let Some(function) = self.breakpoint_ids.remove(id) {
            self.breakpoints.remove(&function).is_some()
        } else {
            false
        }
    }

    /// Get a breakpoint by function name
    pub fn get(&self, function: &str) -> Option<&Breakpoint> {
        self.breakpoints.get(function)
    }

    pub fn get_breakpoint(&self, function: &str) -> Option<&Breakpoint> {
        self.get(function)
    }

    /// Get a mutable breakpoint by function name
    pub fn get_mut(&mut self, function: &str) -> Option<&mut Breakpoint> {
        self.breakpoints.get_mut(function)
    }

    /// Check if execution should break at this function
    /// Returns (should_break, log_output)
    /// - should_break: whether to pause execution
    /// - log_output: optional log message to output
    pub fn should_break_with_context(
        &mut self,
        function: &str,
        evaluator: &dyn ConditionEvaluator,
    ) -> crate::Result<(bool, Option<String>)> {
        let Some(bp) = self.breakpoints.get_mut(function) else {
            return Ok((false, None));
        };

        // Increment hit count
        bp.increment_hit();

        // Check hit condition first (cheapest check)
        if let Some(hit_cond) = &bp.hit_condition {
            if !evaluate_hit_condition(hit_cond, bp.hit_count)? {
                return Ok((false, None));
            }
        }

        // Check expression condition
        if let Some(condition) = &bp.condition {
            if !evaluator.evaluate(condition)? {
                return Ok((false, None));
            }
        }

        // If it's a log point, generate the log message but don't break
        if let Some(log_template) = &bp.log_message {
            let log_output = evaluator.interpolate_log(log_template)?;
            return Ok((false, Some(log_output)));
        }

        // Regular breakpoint - should pause
        Ok((true, None))
    }

    /// Simplified check for backward compatibility
    pub fn should_break(&self, function: &str) -> bool {
        self.breakpoints.contains_key(function)
    }

    /// List all breakpoints
    pub fn list(&self) -> Vec<String> {
        self.breakpoints.keys().cloned().collect()
    }

    /// Get all breakpoints with full details
    pub fn list_detailed(&self) -> Vec<&Breakpoint> {
        self.breakpoints.values().collect()
    }

    pub fn on_hit(
        &mut self,
        function: &str,
        storage: &HashMap<String, String>,
        args: Option<&str>,
    ) -> crate::Result<Option<BreakpointHit>> {
        let Some(bp) = self.breakpoints.get_mut(function) else {
            return Ok(None);
        };

        bp.increment_hit();

        if let Some(hit_cond) = &bp.hit_condition {
            if !evaluate_hit_condition(hit_cond, bp.hit_count)? {
                return Ok(None);
            }
        }

        let log_messages = bp
            .log_message
            .as_deref()
            .map(|template| interpolate_log_message(template, function, storage, args))
            .transpose()?
            .into_iter()
            .collect();
        Ok(Some(BreakpointHit {
            should_pause: !bp.is_log_point(),
            log_messages,
            pause_reason: (!bp.is_log_point()).then(|| "breakpoint".to_string()),
        }))
    }

    /// Clear all breakpoints
    pub fn clear(&mut self) {
        self.breakpoints.clear();
        self.breakpoint_ids.clear();
    }

    /// Check if there are any breakpoints set
    pub fn is_empty(&self) -> bool {
        self.breakpoints.is_empty()
    }

    /// Get count of breakpoints
    pub fn count(&self) -> usize {
        self.breakpoints.len()
    }

    /// Parse a condition string into a validated condition expression.
    /// This validates syntax but does not evaluate it.
    pub fn parse_condition(s: &str) -> crate::Result<String> {
        let s = s.trim();
        if s.is_empty() {
            return Err(crate::DebuggerError::BreakpointError(
                "Condition cannot be empty".to_string(),
            )
            .into());
        }

        if !contains_comparison_operator(s) {
            return Err(crate::DebuggerError::BreakpointError(format!(
                "Invalid condition '{}': must contain a comparison operator (==, !=, <, >, <=, >=)",
                s
            ))
            .into());
        }

        let Some((op, pos)) = find_operator(s) else {
            return Err(crate::DebuggerError::BreakpointError(
                "Condition must contain a comparison operator".to_string(),
            )
            .into());
        };
        let lhs = s[..pos].trim();
        let rhs = s[pos + op.len()..].trim();
        if lhs.is_empty() || rhs.is_empty() {
            return Err(crate::DebuggerError::BreakpointError(
                "Condition must include non-empty left and right operands".to_string(),
            )
            .into());
        }

        Ok(s.to_string())
    }

    /// Parse a hit condition string
    pub fn parse_hit_condition(s: &str) -> crate::Result<String> {
        let s = s.trim();
        if s.is_empty() {
            return Err(crate::DebuggerError::BreakpointError(
                "Hit condition cannot be empty".to_string(),
            )
            .into());
        }

        if !is_valid_hit_condition(s) {
            return Err(crate::DebuggerError::BreakpointError(format!(
                "Invalid hit condition '{}': must be number, >N, >=N, ==N, <N, <=N, or %N==0",
                s
            ))
            .into());
        }

        Ok(s.to_string())
    }
}

/// Trait for evaluating conditions against runtime state
pub trait ConditionEvaluator {
    /// Evaluate a condition expression (e.g., "balance > 1000")
    /// Returns true if the condition is met
    fn evaluate(&self, condition: &str) -> crate::Result<bool>;

    /// Interpolate variables in a log message (e.g., "Balance is {balance}")
    fn interpolate_log(&self, template: &str) -> crate::Result<String>;
}

fn interpolate_log_message(
    template: &str,
    function: &str,
    storage: &HashMap<String, String>,
    args: Option<&str>,
) -> crate::Result<String> {
    let mut result = template.to_string();

    for (name, value) in storage {
        let placeholder = format!("{{{}}}", name);
        result = result.replace(&placeholder, value);
    }

    result = result.replace("{function}", function);

    if let Some(args) = args {
        result = result.replace("{args}", args);
        result = result.replace("{arguments}", args);
    }

    Ok(result)
}

/// Evaluate a hit condition against the current hit count
fn evaluate_hit_condition(hit_condition: &str, hit_count: usize) -> crate::Result<bool> {
    let hit_condition = hit_condition.trim();

    // Format: >N, >=N, ==N, <N, <=N, %N==0, or just N (equivalent to >=N)
    if let Some(stripped) = hit_condition.strip_prefix(">=") {
        let n: usize = stripped.trim().parse().map_err(|_| {
            crate::DebuggerError::BreakpointError(format!(
                "Invalid number in hit condition: {}",
                stripped
            ))
        })?;
        return Ok(hit_count >= n);
    }

    if let Some(stripped) = hit_condition.strip_prefix('>') {
        let n: usize = stripped.trim().parse().map_err(|_| {
            crate::DebuggerError::BreakpointError(format!(
                "Invalid number in hit condition: {}",
                stripped
            ))
        })?;
        return Ok(hit_count > n);
    }

    if let Some(stripped) = hit_condition.strip_prefix("==") {
        let n: usize = stripped.trim().parse().map_err(|_| {
            crate::DebuggerError::BreakpointError(format!(
                "Invalid number in hit condition: {}",
                stripped
            ))
        })?;
        return Ok(hit_count == n);
    }

    if let Some(stripped) = hit_condition.strip_prefix("<=") {
        let n: usize = stripped.trim().parse().map_err(|_| {
            crate::DebuggerError::BreakpointError(format!(
                "Invalid number in hit condition: {}",
                stripped
            ))
        })?;
        return Ok(hit_count <= n);
    }

    if let Some(stripped) = hit_condition.strip_prefix('<') {
        let n: usize = stripped.trim().parse().map_err(|_| {
            crate::DebuggerError::BreakpointError(format!(
                "Invalid number in hit condition: {}",
                stripped
            ))
        })?;
        return Ok(hit_count < n);
    }

    // Modulo format: %N==0 (break every N hits)
    if hit_condition.contains('%') && hit_condition.contains("==") {
        let parts: Vec<&str> = hit_condition.split('%').collect();
        if parts.len() == 2 {
            let rest: Vec<&str> = parts[1].split("==").collect();
            if rest.len() == 2 {
                let n: usize = rest[0].trim().parse().map_err(|_| {
                    crate::DebuggerError::BreakpointError(format!(
                        "Invalid modulo in hit condition: {}",
                        rest[0]
                    ))
                })?;
                let expected: usize = rest[1].trim().parse().map_err(|_| {
                    crate::DebuggerError::BreakpointError(format!(
                        "Invalid value in hit condition: {}",
                        rest[1]
                    ))
                })?;
                if n == 0 {
                    return Err(crate::DebuggerError::BreakpointError(
                        "Modulo cannot be zero".to_string(),
                    )
                    .into());
                }
                return Ok((hit_count % n) == expected);
            }
        }
    }

    // Plain number means "break when hit count >= N"
    if let Ok(n) = hit_condition.parse::<usize>() {
        return Ok(hit_count >= n);
    }

    Err(crate::DebuggerError::BreakpointError(format!(
        "Invalid hit condition format: {}",
        hit_condition
    ))
    .into())
}

fn find_operator(s: &str) -> Option<(&'static str, usize)> {
    [">=", "<=", "==", "!=", ">", "<"]
        .into_iter()
        .find_map(|op| s.find(op).map(|pos| (op, pos)))
}

/// Check if a string contains a comparison operator
fn contains_comparison_operator(s: &str) -> bool {
    s.contains(">=")
        || s.contains("<=")
        || s.contains("==")
        || s.contains("!=")
        || s.contains('>')
        || s.contains('<')
}

/// Validate hit condition format
fn is_valid_hit_condition(s: &str) -> bool {
    let s = s.trim();

    // Check various valid formats
    if s.starts_with(">=")
        || s.starts_with('>')
        || s.starts_with("==")
        || s.starts_with("<=")
        || s.starts_with('<')
    {
        return true;
    }

    // Check modulo format
    if s.contains('%') && s.contains("==") {
        return true;
    }

    // Check if it's just a number
    s.parse::<usize>().is_ok()
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock evaluator for testing
    struct MockEvaluator {
        variables: HashMap<String, i64>,
    }

    impl MockEvaluator {
        fn new() -> Self {
            Self {
                variables: HashMap::new(),
            }
        }

        fn set(&mut self, name: &str, value: i64) {
            self.variables.insert(name.to_string(), value);
        }
    }

    impl ConditionEvaluator for MockEvaluator {
        fn evaluate(&self, condition: &str) -> crate::Result<bool> {
            // Simple parser for "variable operator value"
            let condition = condition.trim();

            // Find operator
            let (var, op, value_str) = if let Some(pos) = condition.find(">=") {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), ">=", rest[2..].trim())
            } else if let Some(pos) = condition.find("<=") {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), "<=", rest[2..].trim())
            } else if let Some(pos) = condition.find("==") {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), "==", rest[2..].trim())
            } else if let Some(pos) = condition.find("!=") {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), "!=", rest[2..].trim())
            } else if let Some(pos) = condition.find('>') {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), ">", rest[1..].trim())
            } else if let Some(pos) = condition.find('<') {
                let (var, rest) = condition.split_at(pos);
                (var.trim(), "<", rest[1..].trim())
            } else {
                return Err(crate::DebuggerError::BreakpointError(format!(
                    "No operator found in condition: {}",
                    condition
                ))
                .into());
            };

            let var_value = self.variables.get(var).ok_or_else(|| {
                crate::DebuggerError::BreakpointError(format!("Variable '{}' not found", var))
            })?;

            let compare_value: i64 = value_str.parse().map_err(|_| {
                crate::DebuggerError::BreakpointError(format!("Invalid number: {}", value_str))
            })?;

            let result = match op {
                ">" => var_value > &compare_value,
                ">=" => var_value >= &compare_value,
                "<" => var_value < &compare_value,
                "<=" => var_value <= &compare_value,
                "==" => var_value == &compare_value,
                "!=" => var_value != &compare_value,
                _ => false,
            };

            Ok(result)
        }

        fn interpolate_log(&self, template: &str) -> crate::Result<String> {
            let mut result = template.to_string();

            // Replace {variable} with values
            for (name, value) in &self.variables {
                let placeholder = format!("{{{}}}", name);
                result = result.replace(&placeholder, &value.to_string());
            }

            Ok(result)
        }
    }

    #[test]
    fn test_simple_breakpoint() {
        let mut manager = BreakpointManager::new();
        manager.add("transfer");
        assert!(manager.should_break("transfer"));
        assert!(!manager.should_break("mint"));
    }

    #[test]
    fn test_conditional_breakpoint() {
        let mut manager = BreakpointManager::new();
        let mut evaluator = MockEvaluator::new();
        evaluator.set("balance", 1500);

        let bp = Breakpoint::with_condition("transfer".to_string(), "balance > 1000".to_string());
        manager.set(bp);

        let (should_break, log) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break);
        assert!(log.is_none());

        // Change balance to fail condition
        evaluator.set("balance", 500);
        let (should_break, log) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);
        assert!(log.is_none());
    }

    #[test]
    fn test_hit_condition_greater_than() {
        let mut manager = BreakpointManager::new();
        let evaluator = MockEvaluator::new();

        let bp = Breakpoint::with_hit_condition("transfer".to_string(), ">2".to_string());
        manager.set(bp);

        // First two hits should not break
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);

        // Third hit should break
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break);
    }

    #[test]
    fn test_hit_condition_equals() {
        let mut manager = BreakpointManager::new();
        let evaluator = MockEvaluator::new();

        let bp = Breakpoint::with_hit_condition("transfer".to_string(), "==3".to_string());
        manager.set(bp);

        // First two hits should not break
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);

        // Third hit should break
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break);

        // Fourth hit should not break
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);
    }

    #[test]
    fn test_hit_condition_modulo() {
        let mut manager = BreakpointManager::new();
        let evaluator = MockEvaluator::new();

        // Break every 2 hits
        let bp = Breakpoint::with_hit_condition("transfer".to_string(), "%2==0".to_string());
        manager.set(bp);

        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break); // Hit 1: 1 % 2 == 1

        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break); // Hit 2: 2 % 2 == 0

        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break); // Hit 3: 3 % 2 == 1

        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break); // Hit 4: 4 % 2 == 0
    }

    #[test]
    fn test_log_point() {
        let mut manager = BreakpointManager::new();
        let mut evaluator = MockEvaluator::new();
        evaluator.set("balance", 1500);
        evaluator.set("amount", 100);

        let bp = Breakpoint::log_point(
            "transfer".to_string(),
            "Transfer {amount} - Balance: {balance}".to_string(),
        );
        manager.set(bp);

        let (should_break, log) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break); // Log points don't break
        assert_eq!(log, Some("Transfer 100 - Balance: 1500".to_string()));
    }

    #[test]
    fn test_on_hit_interpolates_log_message() {
        let mut manager = BreakpointManager::new();
        manager.set(Breakpoint::log_point(
            "transfer".to_string(),
            "Transfer {amount} - Balance: {balance}".to_string(),
        ));

        let storage = HashMap::from([
            ("amount".to_string(), "100".to_string()),
            ("balance".to_string(), "1500".to_string()),
        ]);

        let hit = manager
            .on_hit("transfer", &storage, Some("[100]"))
            .unwrap()
            .unwrap();

        assert!(!hit.should_pause);
        assert_eq!(
            hit.log_messages,
            vec!["Transfer 100 - Balance: 1500".to_string()]
        );
    }

    #[test]
    fn test_on_hit_interpolates_builtin_placeholders() {
        let mut manager = BreakpointManager::new();
        manager.set(Breakpoint::log_point(
            "transfer".to_string(),
            "Function {function} args {args} arguments {arguments}".to_string(),
        ));

        let hit = manager
            .on_hit("transfer", &HashMap::new(), Some("[1,2]"))
            .unwrap()
            .unwrap();

        assert_eq!(
            hit.log_messages,
            vec!["Function transfer args [1,2] arguments [1,2]".to_string()]
        );
    }

    #[test]
    fn test_combined_conditions() {
        let mut manager = BreakpointManager::new();
        let mut evaluator = MockEvaluator::new();
        evaluator.set("balance", 1500);

        let mut bp =
            Breakpoint::with_condition("transfer".to_string(), "balance > 1000".to_string());
        bp.hit_condition = Some(">1".to_string());
        manager.set(bp);

        // First hit: hit_condition fails (not > 1 yet)
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);

        // Second hit: hit_condition passes, expression condition passes
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(should_break);

        // Third hit with low balance: hit_condition passes, expression fails
        evaluator.set("balance", 500);
        let (should_break, _) = manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert!(!should_break);
    }

    #[test]
    fn test_remove_breakpoint() {
        let mut manager = BreakpointManager::new();
        manager.add("transfer");
        assert!(manager.remove("transfer"));
        assert!(!manager.should_break("transfer"));
        assert!(!manager.remove("transfer")); // Second remove returns false
    }

    #[test]
    fn test_remove_breakpoint_by_id() {
        let mut manager = BreakpointManager::new();
        manager.add_spec(BreakpointSpec {
            id: "bp-1".to_string(),
            function: "transfer".to_string(),
            condition: None,
            hit_condition: None,
            log_message: None,
        });

        assert!(manager.remove_by_id("bp-1"));
        assert!(!manager.should_break("transfer"));
        assert!(!manager.remove_by_id("bp-1"));
    }

    #[test]
    fn test_set_replaces_stale_id_index_for_same_function() {
        let mut manager = BreakpointManager::new();
        manager.add_spec(BreakpointSpec {
            id: "bp-1".to_string(),
            function: "transfer".to_string(),
            condition: None,
            hit_condition: None,
            log_message: None,
        });
        manager.add_spec(BreakpointSpec {
            id: "bp-2".to_string(),
            function: "transfer".to_string(),
            condition: None,
            hit_condition: None,
            log_message: None,
        });

        assert!(!manager.remove_by_id("bp-1"));
        assert!(manager.remove_by_id("bp-2"));
        assert!(!manager.should_break("transfer"));
    }

    #[test]
    fn test_list_breakpoints() {
        let mut manager = BreakpointManager::new();
        manager.add("transfer");
        manager.add("mint");
        let list = manager.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"transfer".to_string()));
        assert!(list.contains(&"mint".to_string()));
    }

    #[test]
    fn test_parse_condition_missing_operator_fails() {
        let result = BreakpointManager::parse_condition("balance 1000");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("comparison operator"));
    }

    #[test]
    fn test_parse_condition_missing_lhs_fails() {
        let result = BreakpointManager::parse_condition("> 1000");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("left and right operands"));
    }

    #[test]
    fn test_parse_condition_missing_rhs_fails() {
        let result = BreakpointManager::parse_condition("balance > ");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("left and right operands"));
    }

    #[test]
    fn test_parse_condition_validation() {
        // Valid conditions
        assert!(BreakpointManager::parse_condition("balance > 1000").is_ok());
        assert!(BreakpointManager::parse_condition("x == 5").is_ok());
        assert!(BreakpointManager::parse_condition("count >= 10").is_ok());

        // Invalid conditions
        assert!(BreakpointManager::parse_condition("").is_err());
        assert!(BreakpointManager::parse_condition("just_a_variable").is_err());
    }

    #[test]
    fn test_parse_hit_condition_validation() {
        // Valid hit conditions
        assert!(BreakpointManager::parse_hit_condition(">5").is_ok());
        assert!(BreakpointManager::parse_hit_condition(">=10").is_ok());
        assert!(BreakpointManager::parse_hit_condition("==3").is_ok());
        assert!(BreakpointManager::parse_hit_condition("%2==0").is_ok());
        assert!(BreakpointManager::parse_hit_condition("5").is_ok());

        // Invalid hit conditions
        assert!(BreakpointManager::parse_hit_condition("").is_err());
        assert!(BreakpointManager::parse_hit_condition("invalid").is_err());
    }

    #[test]
    fn test_hit_count_increments() {
        let mut manager = BreakpointManager::new();
        manager.add("transfer");

        let evaluator = MockEvaluator::new();

        // Check hit count increments
        manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert_eq!(manager.get("transfer").unwrap().hit_count, 1);

        manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert_eq!(manager.get("transfer").unwrap().hit_count, 2);

        manager
            .should_break_with_context("transfer", &evaluator)
            .unwrap();
        assert_eq!(manager.get("transfer").unwrap().hit_count, 3);
    }
}
