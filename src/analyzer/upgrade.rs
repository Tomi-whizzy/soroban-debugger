use crate::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

/// WASM value typess
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
    Unknown,
}

impl fmt::Display for WasmType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WasmType::I32 => write!(f, "i32"),
            WasmType::I64 => write!(f, "i64"),
            WasmType::F32 => write!(f, "f32"),
            WasmType::F64 => write!(f, "f64"),
            WasmType::V128 => write!(f, "v128"),
            WasmType::FuncRef => write!(f, "funcref"),
            WasmType::ExternRef => write!(f, "externref"),
            WasmType::Unknown => write!(f, "?"),
        }
    }
}

/// A function signature extracted from a WASM module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub name: String,
    pub params: Vec<WasmType>,
    pub results: Vec<WasmType>,
}

impl fmt::Display for FunctionSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params: Vec<String> = self.params.iter().map(|t| t.to_string()).collect();
        let results: Vec<String> = self.results.iter().map(|t| t.to_string()).collect();
        write!(
            f,
            "{}({}) -> [{}]",
            self.name,
            params.join(", "),
            results.join(", ")
        )
    }
}

/// A breaking change detected between two contract versions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BreakingChange {
    FunctionRemoved {
        name: String,
    },
    ParameterCountChanged {
        name: String,
        old_count: usize,
        new_count: usize,
    },
    ParameterTypeChanged {
        name: String,
        index: usize,
        old_type: WasmType,
        new_type: WasmType,
    },
    ReturnTypeChanged {
        name: String,
        old_types: Vec<WasmType>,
        new_types: Vec<WasmType>,
    },
}

impl fmt::Display for BreakingChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BreakingChange::FunctionRemoved { name } => {
                write!(f, "[REMOVED] {}", name)
            }
            BreakingChange::ParameterCountChanged {
                name,
                old_count,
                new_count,
            } => {
                write!(
                    f,
                    "[PARAMS_CHANGED] {}: {} params -> {} params",
                    name, old_count, new_count
                )
            }
            BreakingChange::ParameterTypeChanged {
                name,
                index,
                old_type,
                new_type,
            } => {
                write!(
                    f,
                    "[PARAM_TYPE] {} param[{}]: {} -> {}",
                    name, index, old_type, new_type
                )
            }
            BreakingChange::ReturnTypeChanged {
                name,
                old_types,
                new_types,
            } => {
                let old: Vec<String> = old_types.iter().map(|t| t.to_string()).collect();
                let new: Vec<String> = new_types.iter().map(|t| t.to_string()).collect();
                write!(
                    f,
                    "[RETURN_TYPE] {}: [{}] -> [{}]",
                    name,
                    old.join(", "),
                    new.join(", ")
                )
            }
        }
    }
}

/// A non-breaking change detected between two contract versions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NonBreakingChange {
    FunctionAdded { name: String },
}

impl fmt::Display for NonBreakingChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NonBreakingChange::FunctionAdded { name } => write!(f, "[ADDED] {}", name),
        }
    }
}

/// Execution result comparison when --test-inputs is provided
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDiff {
    pub function: String,
    pub args: String,
    pub old_result: String,
    pub new_result: String,
    pub outputs_match: bool,
}

/// The full compatibility report
#[derive(Debug, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub is_compatible: bool,
    pub old_wasm_path: String,
    pub new_wasm_path: String,
    pub breaking_changes: Vec<BreakingChange>,
    pub non_breaking_changes: Vec<NonBreakingChange>,
    pub old_functions: Vec<crate::utils::wasm::ContractFunctionSignature>,
    pub new_functions: Vec<crate::utils::wasm::ContractFunctionSignature>,
    pub execution_diffs: Vec<ExecutionDiff>,
}

pub struct UpgradeAnalyzer;

impl UpgradeAnalyzer {
    /// Analyze two WASM binaries and produce a compatibility report
    pub fn analyze(
        old_wasm: &[u8],
        new_wasm: &[u8],
        old_path: &str,
        new_path: &str,
        execution_diffs: Vec<ExecutionDiff>,
    ) -> Result<CompatibilityReport> {
        let old_functions = crate::utils::wasm::parse_function_signatures(old_wasm)?;
        let new_functions = crate::utils::wasm::parse_function_signatures(new_wasm)?;

        let (breaking_changes, non_breaking_changes) =
            Self::diff_signatures(&old_functions, &new_functions);

        let has_execution_mismatches = execution_diffs.iter().any(|d| !d.outputs_match);
        let is_compatible = breaking_changes.is_empty() && !has_execution_mismatches;

        Ok(CompatibilityReport {
            is_compatible,
            old_wasm_path: old_path.to_string(),
            new_wasm_path: new_path.to_string(),
            breaking_changes,
            non_breaking_changes,
            old_functions,
            new_functions,
            execution_diffs,
        })
    }

    /// Compute breaking and non-breaking changes between two sets of function signatures
    fn diff_signatures(
        old: &[crate::utils::wasm::ContractFunctionSignature],
        new: &[crate::utils::wasm::ContractFunctionSignature],
    ) -> (Vec<BreakingChange>, Vec<NonBreakingChange>) {
        use std::collections::BTreeMap;

        let old_by_name: BTreeMap<&str, &crate::utils::wasm::ContractFunctionSignature> =
            old.iter().map(|sig| (sig.name.as_str(), sig)).collect();
        let new_by_name: BTreeMap<&str, &crate::utils::wasm::ContractFunctionSignature> =
            new.iter().map(|sig| (sig.name.as_str(), sig)).collect();

        let mut breaking = Vec::new();
        let mut non_breaking = Vec::new();

        for name in old_by_name.keys() {
            if !new_by_name.contains_key(name) {
                breaking.push(BreakingChange::FunctionRemoved {
                    name: (*name).to_string(),
                });
            }
        }

        for name in new_by_name.keys() {
            if !old_by_name.contains_key(name) {
                non_breaking.push(NonBreakingChange::FunctionAdded {
                    name: (*name).to_string(),
                });
            }
        }

        for (name, old_sig) in &old_by_name {
            let Some(new_sig) = new_by_name.get(name) else {
                continue;
            };

            if old_sig.params.len() != new_sig.params.len() {
                breaking.push(BreakingChange::ParameterCountChanged {
                    name: (*name).to_string(),
                    old_count: old_sig.params.len(),
                    new_count: new_sig.params.len(),
                });
                continue;
            }

            for (idx, (old_param, new_param)) in
                old_sig.params.iter().zip(new_sig.params.iter()).enumerate()
            {
                if old_param.type_name != new_param.type_name {
                    breaking.push(BreakingChange::ParameterTypeChanged {
                        name: (*name).to_string(),
                        index: idx,
                        old_type: parse_contract_type_to_wasm_type(&old_param.type_name),
                        new_type: parse_contract_type_to_wasm_type(&new_param.type_name),
                    });
                }
            }

            if old_sig.return_type != new_sig.return_type {
                breaking.push(BreakingChange::ReturnTypeChanged {
                    name: (*name).to_string(),
                    old_types: old_sig
                        .return_type
                        .iter()
                        .map(|t| parse_contract_type_to_wasm_type(t))
                        .collect(),
                    new_types: new_sig
                        .return_type
                        .iter()
                        .map(|t| parse_contract_type_to_wasm_type(t))
                        .collect(),
                });
            }
        }

        (breaking, non_breaking)
    }
}

fn parse_contract_type_to_wasm_type(type_name: &str) -> WasmType {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "i32" => WasmType::I32,
        "i64" => WasmType::I64,
        "f32" => WasmType::F32,
        "f64" => WasmType::F64,
        "v128" => WasmType::V128,
        "funcref" => WasmType::FuncRef,
        "externref" => WasmType::ExternRef,
        _ => WasmType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(name: &str) -> crate::utils::wasm::ContractFunctionSignature {
        crate::utils::wasm::ContractFunctionSignature {
            name: name.to_string(),
            params: Vec::new(),
            return_type: None,
        }
    }

    #[test]
    fn test_diff_signatures_no_changes() {
        let sig = sig("test");
        let (breaking, non_breaking) = UpgradeAnalyzer::diff_signatures(
            std::slice::from_ref(&sig),
            std::slice::from_ref(&sig),
        );
        assert!(breaking.is_empty());
        assert!(non_breaking.is_empty());
    }

    #[test]
    fn test_diff_signatures_removed_and_added() {
        let sig1 = sig("foo");
        let sig2 = sig("bar");

        let (breaking, non_breaking) = UpgradeAnalyzer::diff_signatures(&[sig1], &[sig2]);

        assert!(breaking.iter().any(|change| matches!(
            change,
            BreakingChange::FunctionRemoved { name } if name == "foo"
        )));
        assert!(non_breaking.iter().any(|change| matches!(
            change,
            NonBreakingChange::FunctionAdded { name } if name == "bar"
        )));
    }

    #[test]
    fn test_diff_signatures_param_and_return_changes() {
        let old = crate::utils::wasm::ContractFunctionSignature {
            name: "transfer".to_string(),
            params: vec![crate::utils::wasm::FunctionParam {
                name: "amount".to_string(),
                type_name: "i64".to_string(),
            }],
            return_type: Some("i64".to_string()),
        };
        let new = crate::utils::wasm::ContractFunctionSignature {
            name: "transfer".to_string(),
            params: vec![crate::utils::wasm::FunctionParam {
                name: "amount".to_string(),
                type_name: "i32".to_string(),
            }],
            return_type: Some("i32".to_string()),
        };

        let (breaking, non_breaking) = UpgradeAnalyzer::diff_signatures(&[old], &[new]);

        assert!(non_breaking.is_empty());
        assert!(breaking.iter().any(|change| matches!(
            change,
            BreakingChange::ParameterTypeChanged { name, index, .. } if name == "transfer" && *index == 0
        )));
        assert!(breaking.iter().any(|change| matches!(
            change,
            BreakingChange::ReturnTypeChanged { name, .. } if name == "transfer"
        )));
    }

    #[test]
    fn test_diff_signatures_param_count_changed() {
        let old = crate::utils::wasm::ContractFunctionSignature {
            name: "mint".to_string(),
            params: vec![crate::utils::wasm::FunctionParam {
                name: "to".to_string(),
                type_name: "Address".to_string(),
            }],
            return_type: None,
        };
        let new = crate::utils::wasm::ContractFunctionSignature {
            name: "mint".to_string(),
            params: vec![
                crate::utils::wasm::FunctionParam {
                    name: "to".to_string(),
                    type_name: "Address".to_string(),
                },
                crate::utils::wasm::FunctionParam {
                    name: "amount".to_string(),
                    type_name: "i64".to_string(),
                },
            ],
            return_type: None,
        };

        let (breaking, _) = UpgradeAnalyzer::diff_signatures(&[old], &[new]);

        assert!(breaking.iter().any(|change| matches!(
            change,
            BreakingChange::ParameterCountChanged { name, old_count, new_count }
                if name == "mint" && *old_count == 1 && *new_count == 2
        )));
    }
}
