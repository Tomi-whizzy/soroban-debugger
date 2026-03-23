//! Soroban contract executor — public façade for the runtime sub-modules.
//!
//! [`ContractExecutor`] is the main entry-point for all contract execution.
//! Internally it delegates to four focused sub-modules:
//!
//! - [`super::loader`]  — WASM loading and environment bootstrap.
//! - [`super::parser`]  — Argument parsing and type-aware normalisation.
//! - [`super::invoker`] — Function invocation with timeout protection.
//! - [`super::result`]  — Result types and formatting helpers.

use crate::inspector::budget::MemorySummary;
use crate::runtime::mocking::{MockCallLogEntry, MockContractDispatcher, MockRegistry};
use crate::utils::arguments::ArgumentParser;
use crate::{DebuggerError, Result};

use soroban_env_host::Host;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use tracing::info;

// ── re-exports so callers never need to import sub-modules directly ───────────
pub use crate::runtime::mocking::MockCallLogEntry as MockCallEntry;
pub use crate::runtime::result::{ExecutionRecord, InstructionCounts, StorageSnapshot};

/// Executes Soroban contracts in a test environment.
pub struct ContractExecutor {
    env: Env,
    contract_address: Address,
    last_execution: Option<ExecutionRecord>,
    last_memory_summary: Option<MemorySummary>,
    mock_registry: Arc<Mutex<MockRegistry>>,
    wasm_bytes: Vec<u8>,
    timeout_secs: u64,
    error_db: crate::debugger::error_db::ErrorDatabase,
}

impl ContractExecutor {
    /// Create a new contract executor by loading and registering `wasm`.
    #[tracing::instrument(skip_all)]
    pub fn new(wasm: Vec<u8>) -> Result<Self> {
        let loaded = crate::runtime::loader::load_contract(&wasm)?;
        Ok(Self {
            env: loaded.env,
            contract_address: loaded.contract_address,
            last_execution: None,
            last_memory_summary: None,
            mock_registry: Arc::new(Mutex::new(MockRegistry::default())),
            wasm_bytes: wasm,
            timeout_secs: 30,
            error_db: loaded.error_db,
        })
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    pub fn set_timeout(&mut self, secs: u64) {
        self.timeout_secs = secs;
    }

    /// Enable auth mocking for interactive/test-like execution flows (e.g. REPL).
    pub fn enable_mock_all_auths(&self) {
        self.env.mock_all_auths();
    }

    /// Generate a test account address (StrKey) for REPL shorthand aliases.
    pub fn generate_repl_account_strkey(&self) -> Result<String> {
        let addr = Address::generate(&self.env);
        let debug = format!("{:?}", addr);
        for token in debug
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|s| !s.is_empty())
        {
            if (token.starts_with('G') || token.starts_with('C')) && token.len() >= 10 {
                return Ok(token.to_string());
            }
        }
        Err(DebuggerError::ExecutionError(format!(
            "Failed to format generated REPL address alias (debug={debug})"
        ))
        .into())
    }

    /// Execute a contract function.
    #[tracing::instrument(skip(self), fields(function = function))]
    pub fn execute(&mut self, function: &str, args: Option<&str>) -> Result<String> {
        // 1. Validate function exists in the WASM export section.
        let exported = crate::utils::wasm::parse_functions(&self.wasm_bytes)?;
        if !exported.contains(&function.to_string()) {
            return Err(DebuggerError::InvalidFunction(function.to_string()).into());
        }

        // 2. Parse arguments.
        let parsed_args = match args {
            Some(json) => {
                crate::runtime::parser::parse_args(&self.env, &self.wasm_bytes, function, json)?
            }
            None => vec![],
        };

        // 3. Invoke and capture the result.
        let storage_fn = || self.get_storage_snapshot();
        let (display, record) = crate::runtime::invoker::invoke_function(
            &self.env,
            &self.contract_address,
            &self.error_db,
            function,
            parsed_args,
            self.timeout_secs,
            storage_fn,
        )?;

        self.last_execution = Some(record);
        Ok(display)
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    pub fn last_execution(&self) -> Option<&ExecutionRecord> {
        self.last_execution.as_ref()
    }
    pub fn last_memory_summary(&self) -> Option<&MemorySummary> {
        self.last_memory_summary.as_ref()
    }

    pub fn set_initial_storage(&mut self, storage_json: String) -> Result<()> {
        #[derive(Debug, Clone, Copy)]
        enum Durability {
            Instance,
            Persistent,
            Temporary,
        }

        fn is_typed_annotation(value: &serde_json::Value) -> bool {
            matches!(
                value,
                serde_json::Value::Object(obj) if obj.get("type").is_some() && obj.get("value").is_some()
            )
        }

        fn normalize_numbers(value: &serde_json::Value) -> Result<serde_json::Value> {
            use serde_json::Value;

            if is_typed_annotation(value) {
                return Ok(value.clone());
            }

            match value {
                Value::Null | Value::Bool(_) | Value::String(_) => Ok(value.clone()),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        Ok(serde_json::json!({ "type": "i64", "value": i }))
                    } else if let Some(u) = n.as_u64() {
                        if u <= i64::MAX as u64 {
                            Ok(serde_json::json!({ "type": "i64", "value": u as i64 }))
                        } else {
                            Ok(serde_json::json!({ "type": "u64", "value": u }))
                        }
                    } else {
                        Err(DebuggerError::StorageError(
                            "Floating-point numbers are not supported in --storage".to_string(),
                        )
                        .into())
                    }
                }
                Value::Array(arr) => {
                    let mut out = Vec::with_capacity(arr.len());
                    for item in arr {
                        out.push(normalize_numbers(item)?);
                    }
                    Ok(Value::Array(out))
                }
                Value::Object(map) => {
                    let mut out = serde_json::Map::new();
                    for (k, v) in map {
                        out.insert(k.clone(), normalize_numbers(v)?);
                    }
                    Ok(Value::Object(out))
                }
            }
        }

        fn parse_one_val(env: &Env, value: &serde_json::Value) -> Result<soroban_sdk::Val> {
            let parser = ArgumentParser::new(env.clone());
            let json = serde_json::to_string(value).map_err(|e| {
                DebuggerError::StorageError(format!("Failed to serialize storage JSON value: {e}"))
            })?;
            let mut vals = parser.parse_args_string(&json).map_err(|e| {
                DebuggerError::StorageError(format!("Failed to parse storage value: {e}"))
            })?;
            if vals.len() != 1 {
                return Err(DebuggerError::StorageError(format!(
                    "Storage entry must resolve to exactly 1 value, got {}",
                    vals.len()
                ))
                .into());
            }
            Ok(vals.remove(0))
        }

        fn parse_durability(raw: Option<&serde_json::Value>) -> Result<Durability> {
            let Some(v) = raw else {
                return Ok(Durability::Instance);
            };
            let Some(s) = v.as_str() else {
                return Err(DebuggerError::StorageError(
                    "durability must be a string: instance|persistent|temporary".to_string(),
                )
                .into());
            };
            match s {
                "instance" => Ok(Durability::Instance),
                "persistent" => Ok(Durability::Persistent),
                "temporary" => Ok(Durability::Temporary),
                other => Err(DebuggerError::StorageError(format!(
                    "Unsupported durability '{other}'. Use instance|persistent|temporary."
                ))
                .into()),
            }
        }

        info!("Setting initial storage");
        let root: serde_json::Value = serde_json::from_str(&storage_json).map_err(|e| {
            DebuggerError::StorageError(format!("Failed to parse initial storage JSON: {e}"))
        })?;

        let mut entries: Vec<(Durability, soroban_sdk::Val, soroban_sdk::Val)> = Vec::new();

        match root {
            serde_json::Value::Object(map) => {
                if let Some(entries_field) = map.get("entries") {
                    if entries_field.is_object() {
                        return Err(DebuggerError::StorageError(
                            "Unsupported --storage format: looks like an exported snapshot. Use a plain object mapping keys to values, e.g. {\"c\": 41}, or use the list form [{\"key\":...,\"value\":...}].".to_string(),
                        )
                        .into());
                    }
                }

                for (k, v) in map {
                    let key_json = serde_json::json!({ "type": "symbol", "value": k });
                    let key_val = parse_one_val(&self.env, &key_json)?;
                    let value_json = normalize_numbers(&v)?;
                    let value_val = parse_one_val(&self.env, &value_json)?;
                    entries.push((Durability::Instance, key_val, value_val));
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    let serde_json::Value::Object(obj) = item else {
                        return Err(DebuggerError::StorageError(
                            "Storage list entries must be objects with {key,value[,durability]}"
                                .to_string(),
                        )
                        .into());
                    };
                    let durability = parse_durability(obj.get("durability"))?;
                    let Some(key) = obj.get("key") else {
                        return Err(DebuggerError::StorageError(
                            "Storage entry is missing required field 'key'".to_string(),
                        )
                        .into());
                    };
                    let Some(value) = obj.get("value") else {
                        return Err(DebuggerError::StorageError(
                            "Storage entry is missing required field 'value'".to_string(),
                        )
                        .into());
                    };

                    let key_val = parse_one_val(&self.env, key)?;
                    let value_json = normalize_numbers(value)?;
                    let value_val = parse_one_val(&self.env, &value_json)?;
                    entries.push((durability, key_val, value_val));
                }
            }
            other => {
                return Err(DebuggerError::StorageError(format!(
                    "Unsupported --storage JSON: expected object or array, got {other}"
                ))
                .into())
            }
        }

        let contract_address = self.contract_address.clone();
        self.env.as_contract(&contract_address, || {
            for (durability, key_val, value_val) in entries {
                match durability {
                    Durability::Instance => {
                        self.env.storage().instance().set(&key_val, &value_val);
                    }
                    Durability::Persistent => {
                        self.env.storage().persistent().set(&key_val, &value_val);
                    }
                    Durability::Temporary => {
                        self.env.storage().temporary().set(&key_val, &value_val);
                    }
                }
            }
        });

        Ok(())
    }
    pub fn set_mock_specs(&mut self, specs: &[String]) -> Result<()> {
        let registry = MockRegistry::from_cli_specs(&self.env, specs)?;
        self.set_mock_registry(registry)
    }
    pub fn set_mock_registry(&mut self, registry: MockRegistry) -> Result<()> {
        self.mock_registry = Arc::new(Mutex::new(registry));
        self.install_mock_dispatchers()
    }
    pub fn get_mock_call_log(&self) -> Vec<MockCallLogEntry> {
        self.mock_registry
            .lock()
            .map(|r| r.calls().to_vec())
            .unwrap_or_default()
    }
    pub fn get_instruction_counts(&self) -> Result<InstructionCounts> {
        Ok(InstructionCounts {
            function_counts: Vec::new(),
            total: 0,
        })
    }
    pub fn host(&self) -> &Host {
        self.env.host()
    }
    pub fn get_auth_tree(&self) -> Result<Vec<crate::inspector::auth::AuthNode>> {
        crate::inspector::auth::AuthInspector::get_auth_tree(&self.env)
    }
    pub fn get_events(&self) -> Result<Vec<crate::inspector::events::ContractEvent>> {
        crate::inspector::events::EventInspector::get_events(self.env.host())
    }
    pub fn get_storage_snapshot(&self) -> Result<HashMap<String, String>> {
        Ok(crate::inspector::storage::StorageInspector::capture_snapshot(self.env.host()))
    }
    pub fn get_ledger_snapshot(&self) -> Result<soroban_ledger_snapshot::LedgerSnapshot> {
        Ok(self.env.to_ledger_snapshot())
    }
    pub fn finish(
        &mut self,
    ) -> Result<(
        soroban_env_host::storage::Footprint,
        soroban_env_host::storage::Storage,
    )> {
        let dummy_env = Env::default();
        let dummy_addr = Address::generate(&dummy_env);
        let old_env = std::mem::replace(&mut self.env, dummy_env);
        self.contract_address = dummy_addr;
        let host = old_env.host().clone();
        drop(old_env);
        let (storage, _events) = host.try_finish().map_err(|e| {
            DebuggerError::ExecutionError(format!(
                "Failed to finalize host execution tracking: {:?}",
                e
            ))
        })?;
        Ok((storage.footprint.clone(), storage))
    }
    pub fn snapshot_storage(&self) -> Result<StorageSnapshot> {
        let storage = self
            .env
            .host()
            .with_mut_storage(|s| Ok(s.clone()))
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to snapshot storage: {:?}", e))
            })?;
        Ok(StorageSnapshot { storage })
    }
    pub fn restore_storage(&mut self, snapshot: &StorageSnapshot) -> Result<()> {
        self.env
            .host()
            .with_mut_storage(|s| {
                *s = snapshot.storage.clone();
                Ok(())
            })
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to restore storage: {:?}", e))
            })?;
        info!("Storage state restored (dry-run rollback)");
        Ok(())
    }
    pub fn get_diagnostic_events(&self) -> Result<Vec<soroban_env_host::xdr::ContractEvent>> {
        Ok(self
            .env
            .host()
            .get_diagnostic_events()
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to get diagnostic events: {}", e))
            })?
            .0
            .into_iter()
            .map(|he| he.event)
            .collect())
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn install_mock_dispatchers(&self) -> Result<()> {
        let ids = self
            .mock_registry
            .lock()
            .map(|r| r.mocked_contract_ids())
            .map_err(|_| DebuggerError::ExecutionError("Mock registry lock poisoned".into()))?;

        for contract_id in ids {
            let address = self.parse_contract_address(&contract_id)?;
            let dispatcher =
                MockContractDispatcher::new(contract_id.clone(), Arc::clone(&self.mock_registry))
                    .boxed();
            self.env
                .host()
                .register_test_contract(address.to_object(), dispatcher)
                .map_err(|e| {
                    DebuggerError::ExecutionError(format!(
                        "Failed to register test contract: {}",
                        e
                    ))
                })?;
        }
        Ok(())
    }

    fn parse_contract_address(&self, contract_id: &str) -> Result<Address> {
        catch_unwind(AssertUnwindSafe(|| {
            Address::from_str(&self.env, contract_id)
        }))
        .map_err(|_| {
            DebuggerError::InvalidArguments(format!("Invalid contract id in --mock: {contract_id}"))
                .into()
        })
    }
}
