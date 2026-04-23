use crate::inspector::budget::BudgetInfo;
use crate::inspector::stack::CallFrame;
use crate::debugger::state::PauseReason;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Function call metadata embedded in timeline snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallSnapshot {
    /// Caller address or identifier
    pub caller: String,
    /// Callee function name
    pub callee: String,
    /// Function arguments as strings
    pub arguments: Vec<String>,
    /// Call depth in the call stack
    pub depth: usize,
    /// Function result if available
    pub result: Option<String>,
    /// Function error if any
    pub error: Option<String>,
}

/// A snapshot of the execution state at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    /// Step number in the timeline
    pub step: usize,
    /// Instruction index in the current function
    pub instruction_index: usize,
    /// Function name
    pub function: String,
    /// Call stack at this point
    pub call_stack: Vec<CallFrame>,
    /// Contract storage snapshot
    pub storage: HashMap<String, String>,
    /// Budget usage at this point
    pub budget: BudgetInfo,
    /// Number of events emitted so far
    pub events_count: usize,
    /// Timestamp of the snapshot
    pub timestamp: u128,
    /// Function call metadata if captured at this step
    pub function_call: Option<FunctionCallSnapshot>,
    /// Pause reason classification for this snapshot when execution is paused
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_reason: Option<PauseReason>,
}

/// Manages the timeline of execution snapshots for time-travel debugging.
pub struct TimelineManager {
    /// History of snapshots
    history: Vec<ExecutionSnapshot>,
    /// Current position in history (index in the history vector)
    current_pos: usize,
    /// Maximum history size
    max_history: usize,
}

impl TimelineManager {
    /// Create a new timeline manager
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::new(),
            current_pos: 0,
            max_history,
        }
    }

    /// Add a new snapshot to the history.
    /// If we are currently in a "back-stepped" state, this will truncate
    /// the history after the current position before adding the new snapshot.
    pub fn push(&mut self, snapshot: ExecutionSnapshot) {
        if self.current_pos < self.history.len().saturating_sub(1) {
            self.history.truncate(self.current_pos + 1);
        }

        if self.history.len() >= self.max_history {
            self.history.remove(0);
        } else {
            self.current_pos = self.history.len();
        }

        self.history.push(snapshot);
        self.current_pos = self.history.len() - 1;
    }

    /// Step back in time
    pub fn step_back(&mut self) -> Option<&ExecutionSnapshot> {
        if self.current_pos > 0 {
            self.current_pos -= 1;
            Some(&self.history[self.current_pos])
        } else {
            None
        }
    }

    /// Step forward in time
    pub fn step_forward(&mut self) -> Option<&ExecutionSnapshot> {
        if self.current_pos < self.history.len().saturating_sub(1) {
            self.current_pos += 1;
            Some(&self.history[self.current_pos])
        } else {
            None
        }
    }

    /// Jump to a specific step
    pub fn goto(&mut self, step: usize) -> Option<&ExecutionSnapshot> {
        if let Some(pos) = self.history.iter().position(|s| s.step == step) {
            self.current_pos = pos;
            Some(&self.history[pos])
        } else {
            None
        }
    }

    /// Get the current snapshot
    pub fn current(&self) -> Option<&ExecutionSnapshot> {
        self.history.get(self.current_pos)
    }

    /// Get all snapshots (for visualization)
    pub fn get_history(&self) -> &[ExecutionSnapshot] {
        &self.history
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.history.clear();
        self.current_pos = 0;
    }

    /// Get current position index
    pub fn current_pos(&self) -> usize {
        self.current_pos
    }

    /// Get total number of snapshots
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}
