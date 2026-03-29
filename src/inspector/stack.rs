use std::time::Duration;

/// Represents a single frame in the call stack
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CallFrame {
    pub function: String,
    pub contract_id: Option<String>,
    pub duration: Option<Duration>,
}

/// Tracks and displays the call stack
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CallStackInspector {
    stack: Vec<CallFrame>,
}

impl CallStackInspector {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a function onto the call stack
    pub fn push(&mut self, function: String, contract_id: Option<String>) {
        self.stack.push(CallFrame {
            function,
            contract_id,
            duration: None,
        });
    }

    /// Push a frame with duration
    pub fn push_frame(&mut self, frame: CallFrame) {
        self.stack.push(frame);
    }

    /// Pop a function from the call stack
    pub fn pop(&mut self) -> Option<CallFrame> {
        self.stack.pop()
    }

    /// Get the current call stack
    pub fn get_stack(&self) -> &[CallFrame] {
        &self.stack
    }

    /// Display the call stack.
    ///
    /// Delegates to [`CallStackInspector::display_frames`] so that callers
    /// holding a `MutexGuard` over the state can pass an already-borrowed
    /// slice without any risk of re-acquiring the same lock inside this path.
    pub fn display(&self) {
        Self::display_frames(&self.stack);
    }

    /// Render `frames` to the log output.
    ///
    /// Accepts a plain slice so this function cannot acquire any lock —
    /// callers that already hold a `MutexGuard<DebugState>` can safely pass
    /// `state.call_stack().get_stack()` and the borrow checker enforces that
    /// no second lock acquisition is possible through this path.
    pub fn display_frames(frames: &[CallFrame]) {
        if frames.is_empty() {
            tracing::info!("Call stack is empty");
            return;
        }

        crate::logging::log_display("Call Stack:", crate::logging::LogLevel::Info);
        for (i, frame) in frames.iter().enumerate() {
            let indent = "  ".repeat(i);
            let contract_ctx = if let Some(ref id) = frame.contract_id {
                format!(" [{}]", id)
            } else {
                "".to_string()
            };

            let duration_ctx = if let Some(duration) = frame.duration {
                format!(" ({:?})", duration)
            } else {
                "".to_string()
            };

            if i == frames.len() - 1 {
                crate::logging::log_display(
                    format!(
                        "{}→ {}{}{}",
                        indent, frame.function, contract_ctx, duration_ctx
                    ),
                    crate::logging::LogLevel::Info,
                );
            } else {
                crate::logging::log_display(
                    format!(
                        "{}└─ {}{}{}",
                        indent, frame.function, contract_ctx, duration_ctx
                    ),
                    crate::logging::LogLevel::Info,
                );
            }
        }
    }

    /// Clear the call stack
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}
