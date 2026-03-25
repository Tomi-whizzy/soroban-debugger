use crate::debugger::engine::DebuggerEngine;
use crate::debugger::breakpoint::{BreakpointManager, BreakpointSpec};
use crate::inspector::budget::BudgetInspector;
use crate::server::protocol::{
    BreakpointCapabilities, BreakpointDescriptor, DebugMessage, DebugRequest, DebugResponse,
};
use crate::simulator::SnapshotLoader;
use crate::Result;
use std::fs;
use std::io::BufReader as StdBufReader;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};

pub struct DebugServer {
    engine: Option<DebuggerEngine>,
    token: Option<String>,
    tls_config: Option<ServerConfig>,
    pending_execution: Option<PendingExecution>,
}

struct PendingExecution {
    function: String,
    args: Option<String>,
}

impl DebugServer {
    pub fn new(
        token: Option<String>,
        cert_path: Option<&Path>,
        key_path: Option<&Path>,
    ) -> Result<Self> {
        let tls_config = if let (Some(cp), Some(kp)) = (cert_path, key_path) {
            Some(load_tls_config(cp, kp)?)
        } else {
            None
        };

        Ok(Self {
            engine: None,
            token,
            tls_config,
            pending_execution: None,
        })
    }

    pub async fn run(mut self, port: u16) -> Result<()> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| miette::miette!("Failed to bind to {}: {}", addr, e))?;
        info!("Debug server listening on {}", addr);
        if self.token.is_some() && self.tls_config.is_none() {
            warn!(
                "Token authentication is enabled without TLS. Treat this as plaintext transport and \
                 restrict access to trusted network boundaries or add TLS termination."
            );
        }

        let acceptor = self
            .tls_config
            .take()
            .map(|cfg| TlsAcceptor::from(Arc::new(cfg)));

        loop {
            let (stream, addr) = listener
                .accept()
                .await
                .map_err(|e| miette::miette!("Failed to accept connection: {}", e))?;
            info!("New connection from {}", addr);

            if let Some(ref acceptor) = acceptor {
                match acceptor.accept(stream).await {
                    Ok(tls_stream) => {
                        if let Err(e) = self.handle_single_connection(tls_stream).await {
                            error!("TLS connection error: {}", e);
                        }
                    }
                    Err(e) => error!("TLS accept error: {}", e),
                }
            } else if let Err(e) = self.handle_single_connection(stream).await {
                error!("TCP connection error: {}", e);
            }
        }
    }

    async fn handle_single_connection<S>(&mut self, stream: S) -> Result<()>
    where
        S: tokio::io::AsyncRead + AsyncWriteExt + Unpin,
    {
        let mut authenticated = self.token.is_none();
        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| miette::miette!("Failed to read from stream: {}", e))?;
            if n == 0 {
                break;
            }

            let message: DebugMessage = match serde_json::from_str(line.trim_end()) {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("Failed to parse request: {}", e);
                    continue;
                }
            };
            let Some(request) = message.request else {
                warn!("Received message without request");
                continue;
            };
            info!("Received request: {}", summarize_request(&request));

            if matches!(request, DebugRequest::Ping) {
                let response = DebugMessage::response(message.id, DebugResponse::Pong);
                send_response(&mut writer, response).await?;
                continue;
            }

            if !authenticated {
                if let DebugRequest::Authenticate { token } = request {
                    let success = self.token.as_deref().map(|t| t == token).unwrap_or(true);
                    authenticated = success;
                    let response = DebugResponse::Authenticated {
                        success,
                        message: if success {
                            "Authentication successful".to_string()
                        } else {
                            "Authentication failed".to_string()
                        },
                    };
                    let response = DebugMessage::response(message.id, response);
                    send_response(&mut writer, response).await?;
                    if !success {
                        return Ok(());
                    }
                    continue;
                }

                let response = DebugMessage::response(
                    message.id,
                    DebugResponse::Error {
                        message: "Authentication required".to_string(),
                    },
                );
                send_response(&mut writer, response).await?;
                continue;
            }

            let is_disconnect = matches!(&request, DebugRequest::Disconnect);
            let response = match request {
                DebugRequest::Authenticate { .. } => DebugResponse::Authenticated {
                    success: true,
                    message: "Already authenticated".to_string(),
                },
                DebugRequest::LoadContract { contract_path } => match fs::read(&contract_path) {
                    Ok(bytes) => {
                        match crate::runtime::executor::ContractExecutor::new(bytes.clone()) {
                            Ok(executor) => {
                                let mut engine = DebuggerEngine::new(executor, Vec::new());
                                let _ = engine.enable_instruction_debug(&bytes);
                                self.engine = Some(engine);
                                self.pending_execution = None;
                                DebugResponse::ContractLoaded {
                                    size: fs::metadata(&contract_path)
                                        .map(|m| m.len() as usize)
                                        .unwrap_or(0),
                                }
                            }
                            Err(e) => DebugResponse::Error {
                                message: e.to_string(),
                            },
                        }
                    }
                    Err(e) => DebugResponse::Error {
                        message: format!("Failed to read contract {:?}: {}", contract_path, e),
                    },
                },
                DebugRequest::Execute { function, args } => match self.engine.as_mut() {
                    Some(engine) if engine.breakpoints().should_break(&function) => {
                        match current_storage(engine) {
                            Ok(storage) => match engine
                                .breakpoints_mut()
                                .on_hit(&function, &storage, args.as_deref())
                            {
                                Ok(Some(hit)) => {
                                    for message in hit.log_messages {
                                        println!("{message}");
                                    }

                                    if hit.should_pause {
                                        engine.prepare_breakpoint_stop(&function, args.as_deref());
                                        self.pending_execution =
                                            Some(PendingExecution { function, args });
                                        DebugResponse::ExecutionResult {
                                            success: true,
                                            output: String::new(),
                                            error: None,
                                            paused: true,
                                            completed: false,
                                            source_location: None,
                                        }
                                    } else {
                                        execute_without_breakpoints(engine, &function, args)
                                    }
                                }
                                Ok(None) => execute_without_breakpoints(engine, &function, args),
                                Err(e) => DebugResponse::Error {
                                    message: e.to_string(),
                                },
                            },
                            Err(e) => DebugResponse::Error {
                                message: e.to_string(),
                            },
                        }
                    }
                    Some(engine) => execute_without_breakpoints(engine, &function, args),
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::StepIn => match self.engine.as_mut() {
                    Some(engine) => match engine.step_into() {
                        Ok(_) => {
                            let (current_function, step_count) = engine
                                .state()
                                .lock()
                                .map(|state| {
                                    (
                                        state.current_function().map(|s| s.to_string()),
                                        state.step_count() as u64,
                                    )
                                })
                                .unwrap_or((None, 0));
                            DebugResponse::StepResult {
                                paused: engine.is_paused(),
                                current_function,
                                step_count,
                                source_location: None,
                            }
                        }
                        Err(e) => DebugResponse::Error {
                            message: e.to_string(),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::Next => match self.engine.as_mut() {
                    Some(engine) => match engine.step_over() {
                        Ok(_) => {
                            let (current_function, step_count) = engine
                                .state()
                                .lock()
                                .map(|state| {
                                    (
                                        state.current_function().map(|s| s.to_string()),
                                        state.step_count() as u64,
                                    )
                                })
                                .unwrap_or((None, 0));
                            DebugResponse::StepResult {
                                paused: engine.is_paused(),
                                current_function,
                                step_count,
                                source_location: None,
                            }
                        }
                        Err(e) => DebugResponse::Error {
                            message: e.to_string(),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::StepOut => match self.engine.as_mut() {
                    Some(engine) => {
                        // When paused at a function-level breakpoint (pending execution),
                        // step-out means executing the function to completion.
                        if let Some(pending) = self.pending_execution.take() {
                            let (current_function, step_count) = engine
                                .state()
                                .lock()
                                .map(|state| {
                                    (
                                        state.current_function().map(|s| s.to_string()),
                                        state.step_count() as u64,
                                    )
                                })
                                .unwrap_or((None, 0));
                            match engine.execute_without_breakpoints(
                                &pending.function,
                                pending.args.as_deref(),
                            ) {
                                Ok(_) => DebugResponse::StepResult {
                                    paused: false,
                                    current_function,
                                    step_count,
                                    source_location: None,
                                },
                                Err(e) => DebugResponse::Error {
                                    message: e.to_string(),
                                },
                            }
                        } else {
                            match engine.step_out() {
                                Ok(_) => {
                                    let (current_function, step_count) = engine
                                        .state()
                                        .lock()
                                        .map(|state| {
                                            (
                                                state.current_function().map(|s| s.to_string()),
                                                state.step_count() as u64,
                                            )
                                        })
                                        .unwrap_or((None, 0));
                                    DebugResponse::StepResult {
                                        paused: engine.is_paused(),
                                        current_function,
                                        step_count,
                                        source_location: None,
                                    }
                                }
                                Err(e) => DebugResponse::Error {
                                    message: e.to_string(),
                                },
                            }
                        }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::Continue => match self.engine.as_mut() {
                    Some(engine) => {
                        if let Some(pending) = self.pending_execution.take() {
                            match engine.execute_without_breakpoints(
                                &pending.function,
                                pending.args.as_deref(),
                            ) {
                                Ok(output) => DebugResponse::ContinueResult {
                                    completed: true,
                                    output: Some(output),
                                    error: None,
                                    paused: false,
                                    source_location: None,
                                },
                                Err(e) => DebugResponse::ContinueResult {
                                    completed: false,
                                    output: None,
                                    error: Some(e.to_string()),
                                    paused: false,
                                    source_location: None,
                                },
                            }
                        } else {
                            match engine.continue_execution() {
                                Ok(_) => DebugResponse::ContinueResult {
                                    completed: true,
                                    output: None,
                                    error: None,
                                    paused: engine.is_paused(),
                                    source_location: None,
                                },
                                Err(e) => DebugResponse::ContinueResult {
                                    completed: false,
                                    output: None,
                                    error: Some(e.to_string()),
                                    paused: engine.is_paused(),
                                    source_location: None,
                                },
                            }
                        }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::Inspect => match self.engine.as_ref() {
                    Some(engine) => match engine.state().lock() {
                        Ok(state) => {
                            let call_stack = state
                                .call_stack()
                                .get_stack()
                                .iter()
                                .map(|frame| {
                                    let suffix = frame
                                        .contract_id
                                        .as_ref()
                                        .map(|id| format!(" [{}]", id))
                                        .unwrap_or_default();
                                    format!("{}{}", frame.function, suffix)
                                })
                                .collect();
                            DebugResponse::InspectionResult {
                                function: state.current_function().map(|s| s.to_string()),
                                args: state.current_args().map(|s| s.to_string()),
                                step_count: state.step_count() as u64,
                                paused: engine.is_paused(),
                                call_stack,
                                source_location: None,
                            }
                        }
                        Err(e) => DebugResponse::Error {
                            message: format!("Failed to acquire state lock: {}", e),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::GetStorage => match self.engine.as_ref() {
                    Some(engine) => match engine.executor().get_storage_snapshot() {
                        Ok(snapshot) => match serde_json::to_string(&snapshot) {
                            Ok(json) => DebugResponse::StorageState { storage_json: json },
                            Err(e) => DebugResponse::Error {
                                message: format!("Failed to serialize storage snapshot: {}", e),
                            },
                        },
                        Err(e) => DebugResponse::Error {
                            message: e.to_string(),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::GetStack => match self.engine.as_ref() {
                    Some(engine) => match engine.state().lock() {
                        Ok(state) => {
                            let stack = state
                                .call_stack()
                                .get_stack()
                                .iter()
                                .map(|frame| {
                                    let suffix = frame
                                        .contract_id
                                        .as_ref()
                                        .map(|id| format!(" [{}]", id))
                                        .unwrap_or_default();
                                    format!("{}{}", frame.function, suffix)
                                })
                                .collect();
                            DebugResponse::CallStack { stack }
                        }
                        Err(e) => DebugResponse::Error {
                            message: format!("Failed to acquire state lock: {}", e),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::GetBudget => match self.engine.as_ref() {
                    Some(engine) => {
                        let info = BudgetInspector::get_cpu_usage(engine.executor().host());
                        DebugResponse::BudgetInfo {
                            cpu_instructions: info.cpu_instructions,
                            memory_bytes: info.memory_bytes,
                        }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::SetBreakpoint {
                    id,
                    function,
                    condition,
                    hit_condition,
                    log_message,
                } => match self.engine.as_mut() {
                    Some(engine) => {
                        let condition = match condition {
                            Some(condition) => match BreakpointManager::parse_condition(&condition) {
                                Ok(condition) => Some(condition),
                                Err(e) => {
                                    let response = DebugMessage::response(
                                        message.id,
                                        DebugResponse::Error {
                                            message: e.to_string(),
                                        },
                                    );
                                    send_response(&mut writer, response).await?;
                                    continue;
                                }
                            },
                            None => None,
                        };
                        let hit_condition = match hit_condition {
                            Some(hit_condition) => {
                                match BreakpointManager::parse_hit_condition(&hit_condition) {
                                    Ok(hit_condition) => Some(hit_condition),
                                    Err(e) => {
                                        let response = DebugMessage::response(
                                            message.id,
                                            DebugResponse::Error {
                                                message: e.to_string(),
                                            },
                                        );
                                        send_response(&mut writer, response).await?;
                                        continue;
                                    }
                                }
                            }
                            None => None,
                        };

                        engine.breakpoints_mut().add(BreakpointSpec {
                            id: id.clone(),
                            function: function.clone(),
                            condition,
                            hit_condition,
                            log_message,
                        });
                        DebugResponse::BreakpointSet { id, function }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::ClearBreakpoint { id } => match self.engine.as_mut() {
                    Some(engine) => {
                        engine.breakpoints_mut().remove(&id);
                        DebugResponse::BreakpointCleared { id }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::ListBreakpoints => match self.engine.as_mut() {
                    Some(engine) => DebugResponse::BreakpointsList {
                        breakpoints: engine
                            .breakpoints_mut()
                            .list_detailed()
                            .into_iter()
                            .map(|breakpoint| BreakpointDescriptor {
                                id: breakpoint.id,
                                function: breakpoint.function,
                                condition: breakpoint.condition,
                                hit_condition: breakpoint.hit_condition,
                                log_message: breakpoint.log_message,
                            })
                            .collect(),
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::GetCapabilities => DebugResponse::Capabilities {
                    breakpoints: BreakpointCapabilities {
                        conditional_breakpoints: true,
                        hit_conditional_breakpoints: true,
                        log_points: true,
                    },
                },
                DebugRequest::SetStorage { storage_json } => match self.engine.as_mut() {
                    Some(engine) => match engine.executor_mut().set_initial_storage(storage_json) {
                        Ok(_) => match engine.executor().get_storage_snapshot() {
                            Ok(snapshot) => match serde_json::to_string(&snapshot) {
                                Ok(json) => DebugResponse::StorageState { storage_json: json },
                                Err(e) => DebugResponse::Error {
                                    message: format!("Failed to serialize storage snapshot: {}", e),
                                },
                            },
                            Err(e) => DebugResponse::Error {
                                message: e.to_string(),
                            },
                        },
                        Err(e) => DebugResponse::Error {
                            message: e.to_string(),
                        },
                    },
                    None => DebugResponse::Error {
                        message: "No contract loaded".to_string(),
                    },
                },
                DebugRequest::LoadSnapshot { snapshot_path } => {
                    match SnapshotLoader::from_file(snapshot_path) {
                        Ok(loader) => match loader.apply_to_environment() {
                            Ok(loaded) => DebugResponse::SnapshotLoaded {
                                summary: loaded.format_summary(),
                            },
                            Err(e) => DebugResponse::Error {
                                message: e.to_string(),
                            },
                        },
                        Err(e) => DebugResponse::Error {
                            message: e.to_string(),
                        },
                    }
                }
                DebugRequest::Evaluate { expression, .. } => match self.engine.as_ref() {
                    Some(engine) => {
                        // First try to look up the expression as a storage key
                        match engine.executor().get_storage_snapshot() {
                            Ok(snapshot) => {
                                if let Some(value) = snapshot.get(&expression) {
                                    let result = serde_json::to_string(value)
                                        .unwrap_or_else(|_| format!("{:?}", value));
                                    DebugResponse::EvaluateResult {
                                        result,
                                        result_type: Some("storage".to_string()),
                                        variables_reference: 0,
                                    }
                                } else {
                                    // Try matching built-in state fields
                                    let state_result = engine.state().lock().ok().and_then(
                                        |state| match expression.as_str() {
                                            "function" | "current_function" => state
                                                .current_function()
                                                .map(|f| (f.to_string(), "string".to_string())),
                                            "args" | "arguments" => state
                                                .current_args()
                                                .map(|a| (a.to_string(), "string".to_string())),
                                            "step_count" | "steps" => Some((
                                                state.step_count().to_string(),
                                                "number".to_string(),
                                            )),
                                            _ => None,
                                        },
                                    );

                                    match state_result {
                                        Some((result, result_type)) => {
                                            DebugResponse::EvaluateResult {
                                                result,
                                                result_type: Some(result_type),
                                                variables_reference: 0,
                                            }
                                        }
                                        None => DebugResponse::Error {
                                            message: format!(
                                                "Cannot evaluate '{}': only storage key lookup \
                                                 and built-in fields (function, args, \
                                                 step_count) are supported",
                                                expression
                                            ),
                                        },
                                    }
                                }
                            }
                            Err(e) => DebugResponse::Error {
                                message: format!("Failed to access storage for evaluation: {}", e),
                            },
                        }
                    }
                    None => DebugResponse::Error {
                        message: "No contract loaded. Evaluation requires an active debug session."
                            .to_string(),
                    },
                },
                DebugRequest::Ping => DebugResponse::Pong,
                DebugRequest::Disconnect => DebugResponse::Disconnected,
            };

            let response = DebugMessage::response(message.id, response);
            send_response(&mut writer, response).await?;

            if is_disconnect {
                break;
            }
        }

        Ok(())
    }
}

async fn send_response<S>(stream: &mut S, response: DebugMessage) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let json = serde_json::to_vec(&response)
        .map_err(|e| miette::miette!("Failed to serialize response: {}", e))?;
    stream
        .write_all(&json)
        .await
        .map_err(|e| miette::miette!("Failed to write response: {}", e))?;
    stream
        .write_all(b"\n")
        .await
        .map_err(|e| miette::miette!("Failed to write response newline: {}", e))?;
    Ok(())
}

fn execute_without_breakpoints(
    engine: &mut DebuggerEngine,
    function: &str,
    args: Option<String>,
) -> DebugResponse {
    match engine.execute_without_breakpoints(function, args.as_deref()) {
        Ok(res) => DebugResponse::ExecutionResult {
            success: true,
            output: res,
            error: None,
            paused: engine.is_paused(),
            completed: true,
            source_location: None,
        },
        Err(e) => DebugResponse::ExecutionResult {
            success: false,
            output: String::new(),
            error: Some(e.to_string()),
            paused: false,
            completed: true,
            source_location: None,
        },
    }
}

fn current_storage(engine: &DebuggerEngine) -> Result<std::collections::HashMap<String, String>> {
    let snapshot = engine.executor().get_storage_snapshot()?;
    Ok(snapshot
        .into_iter()
        .map(|(key, value)| {
            let value = match value {
                serde_json::Value::String(value) => value,
                other => other.to_string(),
            };
            (key, value)
        })
        .collect())
}

fn summarize_request(request: &DebugRequest) -> String {
    match request {
        DebugRequest::Authenticate { token } => {
            format!("Authenticate {{ token: {} }}", redact_secret(token))
        }
        DebugRequest::LoadContract { contract_path } => {
            format!("LoadContract {{ contract_path: {:?} }}", contract_path)
        }
        DebugRequest::Execute { function, args } => {
            format!(
                "Execute {{ function: {:?}, args_present: {} }}",
                function,
                args.is_some()
            )
        }
        DebugRequest::StepIn => "StepIn".to_string(),
        DebugRequest::Next => "Next".to_string(),
        DebugRequest::StepOut => "StepOut".to_string(),
        DebugRequest::Continue => "Continue".to_string(),
        DebugRequest::Inspect => "Inspect".to_string(),
        DebugRequest::GetStorage => "GetStorage".to_string(),
        DebugRequest::GetStack => "GetStack".to_string(),
        DebugRequest::GetBudget => "GetBudget".to_string(),
        DebugRequest::SetBreakpoint { id, function, .. } => {
            format!("SetBreakpoint {{ id: {:?}, function: {:?} }}", id, function)
        }
        DebugRequest::ClearBreakpoint { id } => {
            format!("ClearBreakpoint {{ id: {:?} }}", id)
        }
        DebugRequest::ListBreakpoints => "ListBreakpoints".to_string(),
        DebugRequest::GetCapabilities => "GetCapabilities".to_string(),
        DebugRequest::SetStorage { .. } => "SetStorage { storage_json: <redacted> }".to_string(),
        DebugRequest::LoadSnapshot { snapshot_path } => {
            format!("LoadSnapshot {{ snapshot_path: {:?} }}", snapshot_path)
        }
        DebugRequest::Evaluate { expression, frame_id } => {
            format!(
                "Evaluate {{ expression: {:?}, frame_id: {:?} }}",
                expression, frame_id
            )
        }
        DebugRequest::Ping => "Ping".to_string(),
        DebugRequest::Disconnect => "Disconnect".to_string(),
    }
}

fn redact_secret(secret: &str) -> String {
    if secret.is_empty() {
        "<redacted:empty>".to_string()
    } else {
        format!("<redacted:{} chars>", secret.chars().count())
    }
}

fn load_tls_config(cert_path: &Path, key_path: &Path) -> Result<ServerConfig> {
    let cert_file = fs::File::open(cert_path)
        .map_err(|e| miette::miette!("Failed to open cert file {:?}: {}", cert_path, e))?;
    let mut cert_reader = StdBufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .map_err(|e| miette::miette!("Failed to read certs: {}", e))?
        .into_iter()
        .map(Certificate)
        .collect();

    let key_file = fs::File::open(key_path)
        .map_err(|e| miette::miette!("Failed to open key file {:?}: {}", key_path, e))?;
    let mut key_reader = StdBufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
        .map_err(|e| miette::miette!("Failed to read private keys: {}", e))?;
    if keys.is_empty() {
        return Err(miette::miette!("No private key found"));
    }
    let key = PrivateKey(keys[0].clone());

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| miette::miette!("Failed to setup TLS config: {}", e))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::protocol::DebugRequest;

    #[test]
    fn request_summary_redacts_auth_token() {
        let summary = summarize_request(&DebugRequest::Authenticate {
            token: "super-secret-token".to_string(),
        });
        assert!(summary.contains("<redacted:18 chars>"));
        assert!(!summary.contains("super-secret-token"));
    }

    #[test]
    fn request_summary_redacts_storage_payloads() {
        let summary = summarize_request(&DebugRequest::SetStorage {
            storage_json: "{\"token\":\"secret\"}".to_string(),
        });
        assert!(summary.contains("<redacted>"));
        assert!(!summary.contains("secret"));
    }
}
