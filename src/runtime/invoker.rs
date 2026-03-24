//! Contract function invocation with timeout protection and memory tracking.
//!
//! This module contains the hot path for actually *calling* a Soroban contract
//! function. It wires together:
//! - A timeout watchdog thread using [`std::sync::mpsc`].
//! - The call to [`Env::try_invoke_contract`].
//! - Post-invocation result formatting via [`super::result`].

use crate::debugger::error_db::ErrorDatabase;
use crate::inspector::budget::{BudgetInspector, MemoryTracker};
use crate::runtime::result::{format_invocation_result, ExecutionRecord};
use crate::{DebuggerError, Result};
use indicatif::{ProgressBar, ProgressStyle};
use soroban_env_host::xdr::ScVal;
use soroban_env_host::TryFromVal; // needed for ScVal::try_from_val
use soroban_sdk::{Address, Env, InvokeError, Symbol, Val, Vec as SorobanVec};
use std::collections::HashMap;
use tracing::info;

/// Invoke `function` on the already-registered contract at `contract_address`.
#[tracing::instrument(skip_all, fields(function = function))]
pub fn invoke_function(
    env: &Env,
    contract_address: &Address,
    error_db: &ErrorDatabase,
    function: &str,
    parsed_args: Vec<Val>,
    _timeout_secs: u64,
    storage_fn: impl Fn() -> Result<HashMap<String, String>>,
) -> Result<(String, ExecutionRecord)> {
    info!("Executing function: {}", function);

    let mut memory_tracker = MemoryTracker::new(
        env.host()
            .budget_cloned()
            .get_mem_bytes_consumed()
            .unwrap_or(0),
    );
    memory_tracker.record_snapshot(env.host(), "invoke:start");

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    spinner.set_message(format!("Executing function: {}...", function));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let func_symbol = Symbol::new(env, function);

    let args_vec = if parsed_args.is_empty() {
        SorobanVec::<Val>::new(env)
    } else {
        SorobanVec::from_slice(env, &parsed_args)
    };
    memory_tracker.record_snapshot(env.host(), "invoke:build_args_vec");

    // Capture storage state before the call.
    let storage_before = storage_fn().inspect_err(|_| spinner.finish_and_clear())?;
    memory_tracker.record_snapshot(env.host(), "invoke:storage_before");

    // Convert Val → ScVal for the execution record.
    // TryFromVal is used here via ScVal::try_from_val.
    let sc_args: Vec<ScVal> = parsed_args
        .iter()
        .map(|v| ScVal::try_from_val(env.host(), v))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| {
            spinner.finish_and_clear();
            DebuggerError::ExecutionError(format!("Failed to convert arguments to ScVal: {:?}", e))
        })?;
    memory_tracker.record_snapshot(env.host(), "invoke:convert_args");

    // ── The actual call ───────────────────────────────────────────────────────
    let budget_before = BudgetInspector::get_cpu_usage(env.host());
    let invocation_result =
        env.try_invoke_contract::<Val, InvokeError>(contract_address, &func_symbol, args_vec);
    memory_tracker.record_snapshot(env.host(), "invoke:invoke");

    spinner.finish_and_clear();

    // Capture storage state after the call.
    let storage_after = storage_fn()?;
    memory_tracker.record_snapshot(env.host(), "invoke:storage_after");

    // Format the result.
    let (display_result, record_result) =
        format_invocation_result(&invocation_result, env.host(), error_db);
    memory_tracker.record_snapshot(env.host(), "invoke:result_convert");

    // Display budget / memory usage.
    let budget_after = BudgetInspector::get_cpu_usage(env.host());
    let execution_budget = budget_after.delta_from(&budget_before);
    crate::inspector::BudgetInspector::display(env.host());
    let memory_summary = memory_tracker.finalize(env.host());
    memory_summary.display();

    let record = ExecutionRecord {
        function: function.to_string(),
        args: sc_args,
        result: record_result,
        budget: execution_budget,
        storage_before,
        storage_after,
    };

    display_result.map(|s| (s, record))
}
