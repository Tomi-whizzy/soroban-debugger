use crate::runtime::executor::ContractExecutor;
use crate::server::protocol::{DynamicTraceEvent, DynamicTraceEventKind};
use crate::utils::wasm::{parse_instructions, WasmInstruction};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wasmparser::{Operator, Parser, Payload};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub rule_id: String,
    pub severity: Severity,
    pub location: String,
    pub description: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityReport {
    pub findings: Vec<SecurityFinding>,
}

pub trait SecurityRule {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn analyze_static(&self, _wasm_bytes: &[u8]) -> Result<Vec<SecurityFinding>> {
        Ok(vec![])
    }
    fn analyze_dynamic(
        &self,
        _executor: &ContractExecutor,
        _trace: &[DynamicTraceEvent],
    ) -> Result<Vec<SecurityFinding>> {
        Ok(vec![])
    }
}

pub struct SecurityAnalyzer {
    rules: Vec<Box<dyn SecurityRule>>,
}

impl SecurityAnalyzer {
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(HardcodedAddressRule),
                Box::new(ArithmeticCheckRule),
                Box::new(AuthorizationCheckRule),
                Box::new(ReentrancyPatternRule),
                Box::new(UnboundedIterationRule),
            ],
        }
    }

    pub fn analyze(
        &self,
        wasm_bytes: &[u8],
        executor: Option<&ContractExecutor>,
        trace: Option<&[DynamicTraceEvent]>,
    ) -> Result<SecurityReport> {
        let mut report = SecurityReport::default();

        for rule in &self.rules {
            let static_findings = rule.analyze_static(wasm_bytes)?;
            report.findings.extend(static_findings);

            if let (Some(exec), Some(tr)) = (executor, trace) {
                let dynamic_findings = rule.analyze_dynamic(exec, tr)?;
                report.findings.extend(dynamic_findings);
            }
        }

        Ok(report)
    }
}

impl Default for SecurityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

struct HardcodedAddressRule;
impl SecurityRule for HardcodedAddressRule {
    fn name(&self) -> &str {
        "hardcoded-address"
    }
    fn description(&self) -> &str {
        "Detects hardcoded addresses in WASM bytes."
    }

    fn analyze_static(&self, wasm_bytes: &[u8]) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();

        for payload in Parser::new(0).parse_all(wasm_bytes).flatten() {
            if let Payload::DataSection(reader) = payload {
                for data in reader.into_iter().flatten() {
                    let content = String::from_utf8_lossy(data.data);
                    for word in content.split(|c: char| !c.is_alphanumeric()) {
                        if (word.starts_with('G') || word.starts_with('C')) && word.len() == 56 {
                            findings.push(SecurityFinding {
                                rule_id: self.name().to_string(),
                                severity: Severity::Medium,
                                location: "Data Section".to_string(),
                                description: format!("Found potential hardcoded address: {}", word),
                                remediation: "Use Address::from_str from configuration or function arguments instead of hardcoding.".to_string(),
                            });
                        }
                    }
                }
            }
        }
        Ok(findings)
    }
}

struct ArithmeticCheckRule;
impl SecurityRule for ArithmeticCheckRule {
    fn name(&self) -> &str {
        "arithmetic-overflow"
    }
    fn description(&self) -> &str {
        "Detects potential for unchecked arithmetic overflow."
    }

    fn analyze_static(&self, wasm_bytes: &[u8]) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();
        let instructions = parse_instructions(wasm_bytes);

        for (i, instr) in instructions.iter().enumerate() {
            if Self::is_arithmetic(instr) && !Self::is_guarded(&instructions, i) {
                findings.push(SecurityFinding {
                    rule_id: self.name().to_string(),
                    severity: Severity::Medium,
                    location: format!("Instruction {}", i),
                    description: format!("Unchecked arithmetic operation detected: {:?}", instr),
                    remediation: "Ensure arithmetic operations are guarded with proper bounds checks or overflow handling.".to_string(),
                });
            }
        }

        Ok(findings)
    }
}

impl ArithmeticCheckRule {
    fn is_arithmetic(instr: &WasmInstruction) -> bool {
        matches!(
            instr,
            WasmInstruction::I32Add
                | WasmInstruction::I32Sub
                | WasmInstruction::I32Mul
                | WasmInstruction::I64Add
                | WasmInstruction::I64Sub
                | WasmInstruction::I64Mul
        )
    }

    fn is_guarded(instructions: &[WasmInstruction], idx: usize) -> bool {
        let start = idx.saturating_sub(2);
        let end = (idx + 3).min(instructions.len());

        for instr in &instructions[start..end] {
            if matches!(
                instr,
                WasmInstruction::If | WasmInstruction::BrIf | WasmInstruction::Call
            ) {
                return true;
            }
        }

        false
    }
}

struct AuthorizationCheckRule;
impl SecurityRule for AuthorizationCheckRule {
    fn name(&self) -> &str {
        "missing-auth"
    }
    fn description(&self) -> &str {
        "Detects sensitive flows missing authorization checks."
    }

    fn analyze_dynamic(
        &self,
        _executor: &ContractExecutor,
        trace: &[DynamicTraceEvent],
    ) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();
        let mut auth_seen = false;
        let mut storage_write_seen = false;

        for entry in trace {
            if entry.kind == DynamicTraceEventKind::Authorization {
                auth_seen = true;
            }
            if entry.kind == DynamicTraceEventKind::StorageWrite {
                storage_write_seen = true;
            }
        }

        if storage_write_seen && !auth_seen {
            findings.push(SecurityFinding {
                rule_id: self.name().to_string(),
                severity: Severity::High,
                location: "Dynamic trace".to_string(),
                description: "Storage mutation detected without an authorization event in the execution trace.".to_string(),
                remediation: "Ensure all sensitive functions call `address.require_auth()` before mutating state.".to_string(),
            });
        }

        Ok(findings)
    }
}

struct ReentrancyPatternRule;
impl SecurityRule for ReentrancyPatternRule {
    fn name(&self) -> &str {
        "reentrancy-pattern"
    }
    fn description(&self) -> &str {
        "Detects cross-contract calls followed by storage writes."
    }

    fn analyze_dynamic(
        &self,
        _executor: &ContractExecutor,
        trace: &[DynamicTraceEvent],
    ) -> Result<Vec<SecurityFinding>> {
        let mut findings = Vec::new();
        let mut cross_call_seen = false;

        for entry in trace {
            if entry.kind == DynamicTraceEventKind::CrossContractCall {
                cross_call_seen = true;
            }
            if cross_call_seen && entry.kind == DynamicTraceEventKind::StorageWrite {
                findings.push(SecurityFinding {
                    rule_id: self.name().to_string(),
                    severity: Severity::Medium,
                    location: format!("Trace event {}", entry.sequence),
                    description: "Storage write detected after an external contract call. Possible reentrancy risk.".to_string(),
                    remediation: "Follow checks-effects-interactions: finalize state before external calls.".to_string(),
                });
                break;
            }
        }
        Ok(findings)
    }
}

struct UnboundedIterationRule;
impl SecurityRule for UnboundedIterationRule {
    fn name(&self) -> &str {
        "unbounded-iteration"
    }
    fn description(&self) -> &str {
        "Detects storage-driven loops and unbounded read patterns."
    }

    fn analyze_static(&self, wasm_bytes: &[u8]) -> Result<Vec<SecurityFinding>> {
        let analysis = analyze_unbounded_iteration_static(wasm_bytes);
        if !analysis.suspicious {
            return Ok(Vec::new());
        }

        Ok(vec![SecurityFinding {
            rule_id: self.name().to_string(),
            severity: Severity::High,
            location: "WASM code section".to_string(),
            description: format!(
                "Detected loop(s) with storage-read host calls ({} storage calls while inside loop).",
                analysis.storage_calls_inside_loops
            ),
            remediation: "Bound iteration over storage-backed collections (pagination, explicit limits, or capped batch size).".to_string(),
        }])
    }

    fn analyze_dynamic(
        &self,
        _executor: &ContractExecutor,
        trace: &[DynamicTraceEvent],
    ) -> Result<Vec<SecurityFinding>> {
        Ok(analyze_unbounded_iteration_dynamic(trace)
            .into_iter()
            .map(|mut finding| {
                finding.rule_id = self.name().to_string();
                finding
            })
            .collect())
    }
}

#[derive(Debug, Default)]
struct UnboundedStaticSignal {
    suspicious: bool,
    storage_calls_inside_loops: usize,
}

fn analyze_unbounded_iteration_static(wasm_bytes: &[u8]) -> UnboundedStaticSignal {
    let mut storage_import_indices = HashSet::new();
    let mut imported_func_count = 0u32;
    let mut inside_loop_depth = 0usize;
    let mut signal = UnboundedStaticSignal::default();

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let Ok(payload) = payload else {
            return signal;
        };

        match payload {
            Payload::ImportSection(reader) => {
                for import in reader.flatten() {
                    if let wasmparser::TypeRef::Func(_) = import.ty {
                        if is_storage_read_import(import.module, import.name) {
                            storage_import_indices.insert(imported_func_count);
                        }
                        imported_func_count += 1;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let Ok(mut operators) = body.get_operators_reader() else {
                    continue;
                };
                while !operators.eof() {
                    let Ok(op) = operators.read() else {
                        break;
                    };

                    match op {
                        Operator::Loop { .. } => inside_loop_depth += 1,
                        Operator::End => inside_loop_depth = inside_loop_depth.saturating_sub(1),
                        Operator::Call { function_index }
                            if inside_loop_depth > 0
                                && storage_import_indices.contains(&function_index) =>
                        {
                            signal.storage_calls_inside_loops += 1;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    signal.suspicious = signal.storage_calls_inside_loops > 0;
    signal
}

fn is_storage_read_import(module: &str, name: &str) -> bool {
    let module = module.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();

    (module.contains("env") || module.contains("soroban"))
        && (name.contains("storage")
            && (name.contains("get")
                || name.contains("has")
                || name.contains("next")
                || name.contains("iter")))
}

fn analyze_unbounded_iteration_dynamic(trace: &[DynamicTraceEvent]) -> Option<SecurityFinding> {
    let mut read_key_counts: HashMap<&str, usize> = HashMap::new();
    let mut total_reads = 0usize;

    for entry in trace {
        if entry.kind == DynamicTraceEventKind::StorageRead {
            total_reads += 1;
            if let Some(key) = entry.storage_key.as_deref() {
                *read_key_counts.entry(key).or_insert(0) += 1;
            }
        }
    }

    if total_reads == 0 {
        return None;
    }

    let unique_keys = read_key_counts.len();
    let max_reads_for_one_key = read_key_counts.values().copied().max().unwrap_or(0);
    let likely_unbounded = total_reads >= 64
        && (unique_keys <= total_reads / 4 || max_reads_for_one_key >= 32 || total_reads >= 128);

    if !likely_unbounded {
        return None;
    }

    Some(SecurityFinding {
        rule_id: "unbounded-iteration".to_string(),
        severity: Severity::High,
        location: "Dynamic trace".to_string(),
        description: format!(
            "Observed high storage-read pressure (reads={}, unique_keys={}, max_reads_single_key={}). This pattern is consistent with unbounded or storage-driven iteration.",
            total_reads, unique_keys, max_reads_for_one_key
        ),
        remediation: "Use explicit iteration bounds and pagination for storage traversal to avoid gas-denial risks."
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbounded_iteration_dynamic_flags_high_risk_pattern() {
        let mut trace = Vec::new();
        for i in 0..90usize {
            trace.push(DynamicTraceEvent {
                sequence: i,
                kind: DynamicTraceEventKind::StorageRead,
                message: "contract_storage_get".to_string(),
                function: Some("sweep".to_string()),
                storage_key: Some(format!("user:{}", i % 4)),
                storage_value: None,
            });
        }

        let finding = analyze_unbounded_iteration_dynamic(&trace);
        assert!(finding.is_some());
        assert!(matches!(finding.unwrap().severity, Severity::High));
    }

    #[test]
    fn static_signal_false_for_non_wasm_bytes() {
        let signal = analyze_unbounded_iteration_static(&[1, 2, 3, 4, 5]);
        assert!(!signal.suspicious);
    }
}
