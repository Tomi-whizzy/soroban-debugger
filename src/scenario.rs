use crate::cli::args::{ScenarioArgs, Verbosity};
use crate::debugger::engine::DebuggerEngine;
use crate::logging;
use crate::runtime::executor::ContractExecutor;
use crate::ui::formatter::Formatter;
use crate::{DebuggerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Scenario {
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScenarioStep {
    pub name: Option<String>,
    pub function: String,
    pub args: Option<String>,
    pub expected_return: Option<String>,
    pub expected_storage: Option<HashMap<String, String>>,
}

pub fn run_scenario(args: ScenarioArgs, _verbosity: Verbosity) -> Result<()> {
    println!(
        "{}",
        Formatter::info(format!("Loading scenario file: {:?}", args.scenario))
    );
    let scenario_content = fs::read_to_string(&args.scenario).map_err(|e| {
        DebuggerError::FileError(format!(
            "Failed to read scenario file {:?}: {}",
            args.scenario, e
        ))
    })?;

    let scenario: Scenario = toml::from_str(&scenario_content)
        .map_err(|e| DebuggerError::FileError(format!("Failed to parse scenario TOML: {}", e)))?;

    println!(
        "{}",
        Formatter::info(format!("Loading contract: {:?}", args.contract))
    );
    logging::log_loading_contract(&args.contract.to_string_lossy());

    let wasm_file = crate::utils::wasm::load_wasm(&args.contract).map_err(|e| {
        DebuggerError::WasmLoadError(format!("Failed to load WASM {:?}: {}", args.contract, e))
    })?;

    let mut executor = ContractExecutor::new(wasm_file.bytes)?;

    if let Some(storage_json) = &args.storage {
        // Validate JSON early for clear errors, then pass through to the executor
        // which supports the richer `--storage` formats.
        serde_json::from_str::<serde_json::Value>(storage_json).map_err(|e| {
            DebuggerError::StorageError(format!("Failed to parse initial storage JSON: {}", e))
        })?;
        executor.set_initial_storage(storage_json.clone())?;
    }

    println!(
        "{}",
        Formatter::success(format!(
            "Running {} scenario steps...\n",
            scenario.steps.len()
        ))
    );

    let mut engine = DebuggerEngine::new(executor, vec![]);
    let mut all_passed = true;

    for (i, step) in scenario.steps.iter().enumerate() {
        let step_label = step.name.as_deref().unwrap_or(&step.function);
        println!(
            "{}",
            Formatter::info(format!("Step {}: {}", i + 1, step_label))
        );

        let parsed_args = if let Some(args_json) = &step.args {
            Some(crate::cli::commands::parse_args(args_json)?)
        } else {
            None
        };

        // Execute step
        let result = engine.execute(&step.function, parsed_args.as_deref());

        let mut step_passed = true;

        match result {
            Ok(res) => {
                println!("  Result: {}", res);
                if let Some(expected) = &step.expected_return {
                    if res.trim() == expected.trim() {
                        println!(
                            "  {}",
                            Formatter::success("✓ Return value assertion passed")
                        );
                    } else {
                        println!(
                            "  {}",
                            Formatter::error(format!(
                                "✗ Return value assertion failed! Expected '{}', got '{}'",
                                expected, res
                            ))
                        );
                        step_passed = false;
                    }
                }
            }
            Err(e) => {
                println!(
                    "  {}",
                    Formatter::error(format!("✗ Execution failed: {}", e))
                );
                step_passed = false;
            }
        }

        // Check storage assertions if any and step passed execute
        if step_passed {
            if let Some(expected_storage) = &step.expected_storage {
                let snapshot = engine.executor().get_storage_snapshot()?;
                let mut storage_passed = true;
                for (key, expected_val) in expected_storage {
                    if let Some(actual_val) = snapshot.get(key) {
                        if actual_val.trim() == expected_val.trim() {
                            println!(
                                "  {}",
                                Formatter::success(format!(
                                    "✓ Storage assertion passed for key '{}'",
                                    key
                                ))
                            );
                        } else {
                            println!("  {}", Formatter::error(format!("✗ Storage assertion failed for key '{}'! Expected '{}', got '{}'", key, expected_val, actual_val)));
                            storage_passed = false;
                        }
                    } else {
                        println!(
                            "  {}",
                            Formatter::error(format!(
                                "✗ Storage assertion failed! Key '{}' not found",
                                key
                            ))
                        );
                        storage_passed = false;
                    }
                }
                if !storage_passed {
                    step_passed = false;
                }
            }
        }

        if step_passed {
            println!(
                "{}",
                Formatter::success(format!("Step {} passed.\n", i + 1))
            );
        } else {
            println!(
                "{}",
                Formatter::warning(format!("Step {} failed.\n", i + 1))
            );
            all_passed = false;
            break; // Stop execution on first failure
        }
    }

    if all_passed {
        println!(
            "{}",
            Formatter::success("All scenario steps passed successfully!")
        );
        Ok(())
    } else {
        Err(DebuggerError::ExecutionError("Scenario execution failed".into()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_deserialization() {
        let toml_str = r#"
            [[steps]]
            name = "Init"
            function = "init"
            args = '["admin", 10]'
            expected_return = "()"

            [[steps]]
            name = "Get Counter"
            function = "get"
            expected_return = "1"
            [steps.expected_storage]
            "Counter" = "1"
        "#;

        let scenario: Scenario = toml::from_str(toml_str).unwrap();
        assert_eq!(scenario.steps.len(), 2);

        assert_eq!(scenario.steps[0].name.as_deref(), Some("Init"));
        assert_eq!(scenario.steps[0].function, "init");
        assert_eq!(scenario.steps[0].args.as_deref(), Some("[\"admin\", 10]"));
        assert_eq!(scenario.steps[0].expected_return.as_deref(), Some("()"));
        assert!(scenario.steps[0].expected_storage.is_none());

        assert_eq!(scenario.steps[1].name.as_deref(), Some("Get Counter"));
        assert_eq!(scenario.steps[1].function, "get");
        assert!(scenario.steps[1].args.is_none());
        assert_eq!(scenario.steps[1].expected_return.as_deref(), Some("1"));
        assert_eq!(
            scenario.steps[1]
                .expected_storage
                .as_ref()
                .unwrap()
                .get("Counter")
                .map(|s| s.as_str()),
            Some("1")
        );
    }
}
