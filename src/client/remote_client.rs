use crate::server::protocol::{
    DebugMessage, DebugRequest, DebugResponse, PROTOCOL_MAX_VERSION, PROTOCOL_MIN_VERSION,
};
use crate::{DebuggerError, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use tracing::info;

/// Remote client for connecting to a debug server
#[derive(Debug)]
pub struct RemoteClient {
    stream: BufReader<TcpStream>,
    message_id: u64,
    authenticated: bool,
}

impl RemoteClient {
    /// Connect to a remote debug server
    pub fn connect(addr: &str, token: Option<String>) -> Result<Self> {
        info!("Connecting to debug server at {}", addr);
        let stream = TcpStream::connect(addr).map_err(|e| {
            DebuggerError::NetworkError(format!("Failed to connect to {}: {}", addr, e))
        })?;

        let mut client = Self {
            stream: BufReader::new(stream),
            message_id: 0,
            authenticated: token.is_none(),
        };

        client.handshake("rust-remote-client", env!("CARGO_PKG_VERSION"))?;

        // Authenticate if token is provided
        if let Some(token) = token {
            client.authenticate(&token)?;
        }

        Ok(client)
    }

    /// Perform a protocol handshake and verify compatibility.
    pub fn handshake(&mut self, client_name: &str, client_version: &str) -> Result<u32> {
        let response = self.send_request(DebugRequest::Handshake {
            client_name: client_name.to_string(),
            client_version: client_version.to_string(),
            protocol_min: PROTOCOL_MIN_VERSION,
            protocol_max: PROTOCOL_MAX_VERSION,
        })?;

        match response {
            DebugResponse::HandshakeAck {
                selected_version, ..
            } => Ok(selected_version),
            DebugResponse::IncompatibleProtocol { message, .. } => {
                Err(DebuggerError::ExecutionError(format!(
                    "Incompatible debugger protocol: {}",
                    message
                ))
                .into())
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to Handshake".to_string())
                    .into(),
            ),
        }
    }

    /// Authenticate with the server
    pub fn authenticate(&mut self, token: &str) -> Result<()> {
        let response = self.send_request(DebugRequest::Authenticate {
            token: token.to_string(),
        })?;

        match response {
            DebugResponse::Authenticated { success, message } => {
                if success {
                    self.authenticated = true;
                    info!("Authentication successful");
                    Ok(())
                } else {
                    let sanitized = sanitize_auth_message(&message, token);
                    Err(DebuggerError::ExecutionError(format!(
                        "Authentication failed: {}",
                        sanitized
                    ))
                    .into())
                }
            }
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to authentication".to_string(),
            )
            .into()),
        }
    }

    /// Load a contract on the server
    pub fn load_contract(&mut self, contract_path: &str) -> Result<usize> {
        let response = self.send_request(DebugRequest::LoadContract {
            contract_path: contract_path.to_string(),
        })?;

        match response {
            DebugResponse::ContractLoaded { size } => {
                info!("Contract loaded: {} bytes", size);
                Ok(size)
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to LoadContract".to_string(),
            )
            .into()),
        }
    }

    /// Execute a function on the remote server
    pub fn execute(&mut self, function: &str, args: Option<&str>) -> Result<String> {
        let response = self.send_request(DebugRequest::Execute {
            function: function.to_string(),
            args: args.map(|s| s.to_string()),
        })?;

        match response {
            DebugResponse::ExecutionResult {
                success,
                output,
                error,
                ..
            } => {
                if success {
                    Ok(output)
                } else {
                    Err(DebuggerError::ExecutionError(
                        error.unwrap_or_else(|| "Unknown error".to_string()),
                    )
                    .into())
                }
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to Execute".to_string()).into(),
            ),
        }
    }

    /// Step into next inline/instruction
    pub fn step_in(&mut self) -> Result<(bool, Option<String>, u64)> {
        let response = self.send_request(DebugRequest::StepIn)?;

        match response {
            DebugResponse::StepResult {
                paused,
                current_function,
                step_count,
                ..
            } => Ok((paused, current_function, step_count)),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to StepIn".to_string()).into(),
            ),
        }
    }

    /// Step over current function
    pub fn step_over(&mut self) -> Result<(bool, Option<String>, u64)> {
        let response = self.send_request(DebugRequest::Next)?;

        match response {
            DebugResponse::StepResult {
                paused,
                current_function,
                step_count,
                ..
            } => Ok((paused, current_function, step_count)),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => {
                Err(DebuggerError::ExecutionError("Unexpected response to Next".to_string()).into())
            }
        }
    }

    /// Step out of current function
    pub fn step_out(&mut self) -> Result<(bool, Option<String>, u64)> {
        let response = self.send_request(DebugRequest::StepOut)?;

        match response {
            DebugResponse::StepResult {
                paused,
                current_function,
                step_count,
                ..
            } => Ok((paused, current_function, step_count)),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to StepOut".to_string()).into(),
            ),
        }
    }

    /// Continue execution
    pub fn continue_execution(&mut self) -> Result<bool> {
        let response = self.send_request(DebugRequest::Continue)?;

        match response {
            DebugResponse::ContinueResult { completed, .. } => Ok(completed),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to Continue".to_string()).into(),
            ),
        }
    }

    /// Inspect current state
    pub fn inspect(&mut self) -> Result<(Option<String>, u64, bool, Vec<String>)> {
        let response = self.send_request(DebugRequest::Inspect)?;

        match response {
            DebugResponse::InspectionResult {
                function,
                step_count,
                paused,
                call_stack,
                ..
            } => Ok((function, step_count, paused, call_stack)),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to Inspect".to_string()).into(),
            ),
        }
    }

    /// Get storage state
    pub fn get_storage(&mut self) -> Result<String> {
        let response = self.send_request(DebugRequest::GetStorage)?;

        match response {
            DebugResponse::StorageState { storage_json } => Ok(storage_json),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to GetStorage".to_string(),
            )
            .into()),
        }
    }

    /// Get call stack
    pub fn get_stack(&mut self) -> Result<Vec<String>> {
        let response = self.send_request(DebugRequest::GetStack)?;

        match response {
            DebugResponse::CallStack { stack } => Ok(stack),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to GetStack".to_string()).into(),
            ),
        }
    }

    /// Get budget information
    pub fn get_budget(&mut self) -> Result<(u64, u64)> {
        let response = self.send_request(DebugRequest::GetBudget)?;

        match response {
            DebugResponse::BudgetInfo {
                cpu_instructions,
                memory_bytes,
            } => Ok((cpu_instructions, memory_bytes)),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(
                DebuggerError::ExecutionError("Unexpected response to GetBudget".to_string())
                    .into(),
            ),
        }
    }

    /// Set a breakpoint
    pub fn set_breakpoint(&mut self, function: &str, condition: Option<String>) -> Result<()> {
        let response = self.send_request(DebugRequest::SetBreakpoint {
            id: function.to_string(),
            function: function.to_string(),
            condition: None,
            hit_condition: None,
            log_message: None,
        })?;

        match response {
            DebugResponse::BreakpointSet { .. } => {
                info!("Breakpoint set at {}", function);
                Ok(())
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to SetBreakpoint".to_string(),
            )
            .into()),
        }
    }

    /// Clear a breakpoint
    pub fn clear_breakpoint(&mut self, function: &str) -> Result<()> {
        let response = self.send_request(DebugRequest::ClearBreakpoint {
            id: function.to_string(),
        })?;

        match response {
            DebugResponse::BreakpointCleared { .. } => {
                info!("Breakpoint cleared at {}", function);
                Ok(())
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to ClearBreakpoint".to_string(),
            )
            .into()),
        }
    }

    /// List all breakpoints
    pub fn list_breakpoints(&mut self) -> Result<Vec<String>> {
        let response = self.send_request(DebugRequest::ListBreakpoints)?;

        match response {
            DebugResponse::BreakpointsList { breakpoints } => Ok(breakpoints
                .into_iter()
                .map(|breakpoint| breakpoint.function)
                .collect()),
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to ListBreakpoints".to_string(),
            )
            .into()),
        }
    }

    /// Set initial storage
    pub fn set_storage(&mut self, storage_json: &str) -> Result<()> {
        let response = self.send_request(DebugRequest::SetStorage {
            storage_json: storage_json.to_string(),
        })?;

        match response {
            DebugResponse::StorageState { .. } => {
                info!("Storage set successfully");
                Ok(())
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to SetStorage".to_string(),
            )
            .into()),
        }
    }

    /// Load network snapshot
    pub fn load_snapshot(&mut self, snapshot_path: &str) -> Result<String> {
        let response = self.send_request(DebugRequest::LoadSnapshot {
            snapshot_path: snapshot_path.to_string(),
        })?;

        match response {
            DebugResponse::SnapshotLoaded { summary } => {
                info!("Snapshot loaded: {}", summary);
                Ok(summary)
            }
            DebugResponse::Error { message } => Err(DebuggerError::ExecutionError(message).into()),
            _ => Err(DebuggerError::ExecutionError(
                "Unexpected response to LoadSnapshot".to_string(),
            )
            .into()),
        }
    }

    /// Ping the server
    pub fn ping(&mut self) -> Result<()> {
        let response = self.send_request(DebugRequest::Ping)?;

        match response {
            DebugResponse::Pong => {
                info!("Server responded to ping");
                Ok(())
            }
            _ => {
                Err(DebuggerError::ExecutionError("Unexpected response to Ping".to_string()).into())
            }
        }
    }

    /// Disconnect from the server
    pub fn disconnect(&mut self) -> Result<()> {
        let _ = self.send_request(DebugRequest::Disconnect);
        info!("Disconnected from server");
        Ok(())
    }

    /// Send a request and wait for response
    fn send_request(&mut self, request: DebugRequest) -> Result<DebugResponse> {
        if !self.authenticated
            && !matches!(
                request,
                DebugRequest::Handshake { .. }
                    | DebugRequest::Authenticate { .. }
                    | DebugRequest::Ping
            )
        {
            return Err(DebuggerError::ExecutionError(
                "Not authenticated. Call authenticate() first.".to_string(),
            )
            .into());
        }

        self.message_id += 1;
        let expected_id = self.message_id;
        let message = DebugMessage::request(expected_id, request);

        let request_json = serde_json::to_string(&message)
            .map_err(|e| DebuggerError::FileError(format!("Failed to serialize request: {}", e)))?;

        // Send request
        writeln!(self.stream.get_mut(), "{}", request_json).map_err(|e| {
            DebuggerError::NetworkError(format!("Failed to write to stream: {}", e))
        })?;
        self.stream
            .get_mut()
            .flush()
            .map_err(|e| DebuggerError::NetworkError(format!("Failed to flush stream: {}", e)))?;

        // Read response
        let mut response_line = String::new();
        let n = self
            .stream
            .read_line(&mut response_line)
            .map_err(|e| DebuggerError::NetworkError(format!("Failed to read response: {}", e)))?;
        if n == 0 {
            return Err(DebuggerError::NetworkError("No response from server".to_string()).into());
        }

        parse_response_line(expected_id, response_line.trim_end())
    }
}

fn parse_response_line(expected_id: u64, response_line: &str) -> Result<DebugResponse> {
    let response_message: DebugMessage = serde_json::from_str(response_line)
        .map_err(|e| DebuggerError::FileError(format!("Failed to parse response: {}", e)))?;

    if response_message.id != expected_id {
        return Err(DebuggerError::ExecutionError(format!(
            "Mismatched response id: expected {} got {}",
            expected_id, response_message.id
        ))
        .into());
    }

    response_message.response.ok_or_else(|| {
        DebuggerError::FileError("Response message has no response field".to_string()).into()
    })
}

fn sanitize_auth_message(message: &str, token: &str) -> String {
    if token.is_empty() {
        return message.to_string();
    }

    message.replace(token, "<redacted>")
}

impl Drop for RemoteClient {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::protocol::DebugResponse;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn parse_response_line_rejects_mismatched_ids() {
        let msg = DebugMessage::response(42, DebugResponse::Pong);
        let line = serde_json::to_string(&msg).unwrap();
        let err = parse_response_line(7, &line).unwrap_err();
        assert!(err.to_string().contains("Mismatched response id"));
    }

    #[test]
    fn parse_response_line_accepts_matching_ids() {
        let msg = DebugMessage::response(7, DebugResponse::Pong);
        let line = serde_json::to_string(&msg).unwrap();
        let resp = parse_response_line(7, &line).unwrap();
        assert!(matches!(resp, DebugResponse::Pong));
    }

    #[test]
    fn connect_failure_is_network_error_category() {
        let err = RemoteClient::connect("127.0.0.1:1", None).unwrap_err();
        assert!(err.to_string().contains("Network/transport error"));
    }

    #[test]
    fn reuses_buffered_stream_across_rapid_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener address");

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept client");
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .expect("set read timeout");

            let read_stream = stream.try_clone().expect("clone stream for reader");
            let mut reader = BufReader::new(read_stream);
            let mut writer = stream;

            for id in 1..=7 {
                let mut request_line = String::new();
                let bytes_read = reader.read_line(&mut request_line).expect("read request");
                assert!(bytes_read > 0, "client closed before request {id}");

                let request: DebugMessage =
                    serde_json::from_str(&request_line).expect("parse request");
                assert_eq!(request.id, id);

                let response = match request.request.expect("request payload") {
                    DebugRequest::Handshake { .. } => DebugMessage::response(
                        request.id,
                        DebugResponse::HandshakeAck {
                            selected_version: PROTOCOL_MAX_VERSION,
                            server_name: "test-server".to_string(),
                            server_version: "0.0.0".to_string(),
                        },
                    ),
                    DebugRequest::Ping => DebugMessage::response(request.id, DebugResponse::Pong),
                    DebugRequest::Disconnect => {
                        DebugMessage::response(request.id, DebugResponse::Disconnected)
                    }
                    other => panic!("unexpected request: {other:?}"),
                };

                let response_json = serde_json::to_string(&response).expect("serialize response");
                writer
                    .write_all(format!("{response_json}\n").as_bytes())
                    .expect("write response");
                writer.flush().expect("flush response");
            }
        });

        let mut client = RemoteClient::connect(&addr.to_string(), None).expect("connect client");

        for _ in 0..5 {
            client.ping().expect("ping over persistent stream");
        }

        client.disconnect().expect("disconnect client");
        server.join().expect("server thread");
    }

    #[test]
    fn sanitize_auth_message_redacts_token_echo() {
        let sanitized = sanitize_auth_message(
            "Authentication failed for token super-secret-token",
            "super-secret-token",
        );
        assert!(sanitized.contains("<redacted>"));
        assert!(!sanitized.contains("super-secret-token"));
    }
}
