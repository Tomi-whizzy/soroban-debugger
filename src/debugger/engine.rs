use crate::debugger::breakpoint::BreakpointManager;
use crate::debugger::instruction_pointer::StepMode;
use crate::debugger::source_map::{SourceLocation, SourceMap};
use crate::debugger::state::DebugState;
use crate::debugger::stepper::Stepper;
use crate::plugin::{EventContext, ExecutionEvent};
use crate::runtime::executor::ContractExecutor;
use crate::runtime::instruction::Instruction;
use crate::runtime::instrumentation::Instrumenter;
use crate::Result;
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct StepOverResult {
    pub paused: bool,
    pub location: Option<SourceLocation>,
}

/// Core debugging engine that orchestrates execution and debugging.
pub struct DebuggerEngine {
    executor: ContractExecutor,
    breakpoints: BreakpointManager,
    state: Arc<Mutex<DebugState>>,
    stepper: Stepper,
    instrumenter: Instrumenter,
    source_map: Option<SourceMap>,
    paused: bool,
    instruction_debug_enabled: bool,
}

impl DebuggerEngine {
    /// Create a new debugger engine.
    #[tracing::instrument(skip_all)]
    pub fn new(executor: ContractExecutor, initial_breakpoints: Vec<String>) -> Self {
        let mut breakpoints = BreakpointManager::new();

        for bp in initial_breakpoints {
            breakpoints.add_simple(&bp);
            info!("Breakpoint set at function: {}", bp);
        }

        Self {
            executor,
            breakpoints,
            state: Arc::new(Mutex::new(DebugState::new())),
            stepper: Stepper::new(),
            instrumenter: Instrumenter::new(),
            source_map: None,
            paused: false,
            instruction_debug_enabled: false,
        }
    }

    /// Best-effort DWARF source map loading.
    ///
    /// Missing or malformed debug information does not fail execution; it simply leaves the
    /// engine without source mappings.
    pub fn try_load_source_map(&mut self, wasm_bytes: &[u8]) {
        let mut source_map = SourceMap::new();
        match source_map.load(wasm_bytes) {
            Ok(()) if !source_map.is_empty() => {
                self.source_map = Some(source_map);
            }
            _ => {
                self.source_map = None;
            }
        }
    }

    pub fn source_map(&self) -> Option<&SourceMap> {
        self.source_map.as_ref()
    }

    pub fn source_map_mut(&mut self) -> Option<&mut SourceMap> {
        self.source_map.as_mut()
    }

    pub fn lookup_source_location(&self, wasm_offset: usize) -> Option<SourceLocation> {
        self.source_map.as_ref()?.lookup(wasm_offset)
    }

    /// Enable instruction-level debugging.
    pub fn enable_instruction_debug(&mut self, wasm_bytes: &[u8]) -> Result<()> {
        self.try_load_source_map(wasm_bytes);

        let instructions = self
            .instrumenter
            .parse_instructions(wasm_bytes)
            .map_err(|e| miette::miette!("Failed to parse instructions: {}", e))?
            .to_vec();

        if let Ok(mut state) = self.state.lock() {
            state.set_instructions(instructions);
            state.enable_instruction_debug();
        }

        self.instrumenter.enable();
        self.instruction_debug_enabled = true;
        Ok(())
    }

    pub fn load_source_map(&mut self, wasm_bytes: &[u8]) -> Result<()> {
        let mut source_map = SourceMap::new();
        source_map.load(wasm_bytes)?;
        self.source_map = Some(source_map);
        Ok(())
    }

    /// Disable instruction-level debugging.
    pub fn disable_instruction_debug(&mut self) {
        self.instrumenter.disable();
        self.instrumenter.remove_hook();
        if let Ok(mut state) = self.state.lock() {
            state.disable_instruction_debug();
        }
        self.instruction_debug_enabled = false;
    }

    /// Check if instruction-level debugging is enabled.
    pub fn is_instruction_debug_enabled(&self) -> bool {
        self.instruction_debug_enabled
    }

    /// Execute a contract function with debugging.
    #[tracing::instrument(skip(self), fields(function = function))]
    pub fn execute(&mut self, function: &str, args: Option<&str>) -> Result<String> {
        self.execute_internal(function, args, true)
    }

    pub fn execute_without_breakpoints(
        &mut self,
        function: &str,
        args: Option<&str>,
    ) -> Result<String> {
        self.execute_internal(function, args, false)
    }

    fn execute_internal(
        &mut self,
        function: &str,
        args: Option<&str>,
        check_breakpoints: bool,
    ) -> Result<String> {
        info!("Executing function: {}", function);
        self.paused = false;

        if let Ok(mut state) = self.state.lock() {
            state.set_current_function(function.to_string(), args.map(str::to_string));
            state.call_stack_mut().clear();
            state.call_stack_mut().push(function.to_string(), None);
        }

        let mut plugin_ctx = EventContext::new();
        plugin_ctx.stack_depth = self
            .state
            .lock()
            .map(|s| s.call_stack().get_stack().len())
            .unwrap_or(0);
        plugin_ctx.is_paused = self.paused;
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::BeforeFunctionCall {
                function: function.to_string(),
                args: args.map(str::to_string),
            },
            &mut plugin_ctx,
        );

        let (step_count, current_args) = self
            .state
            .lock()
            .map(|s| (s.step_count(), s.current_args().map(String::from)))
            .unwrap_or((0, None));

        if check_breakpoints {
            if let Some(bp) = self.breakpoints().get_breakpoint(function) {
                let condition = bp.condition.clone();
                let _ = step_count;
                let _ = current_args;
                self.pause_at_function(function, condition);
            }
        }

        let start_time = std::time::Instant::now();
        let result = self.executor.execute(function, args);
        let duration = start_time.elapsed();

        self.update_call_stack(duration)?;

        let event_result = match &result {
            Ok(output) => Ok(output.clone()),
            Err(e) => Err(e.to_string()),
        };
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::AfterFunctionCall {
                function: function.to_string(),
                result: event_result,
                duration,
            },
            &mut plugin_ctx,
        );

        if let Err(ref e) = result {
            tracing::error!("Execution failed: {}", e);
            if let Ok(state) = self.state.lock() {
                state.call_stack().display();
            }
        } else if self.is_paused() {
            if let Ok(state) = self.state.lock() {
                state.call_stack().display();
            }
        }

        result
    }

    pub fn prepare_breakpoint_stop(&mut self, function: &str, args: Option<&str>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_current_function(function.to_string(), args.map(str::to_string));
            state.call_stack_mut().clear();
            state.call_stack_mut().push(function.to_string(), None);
        }

        crate::logging::log_breakpoint(function);
        self.paused = true;

        let mut plugin_ctx = EventContext::new();
        plugin_ctx.stack_depth = 1;
        plugin_ctx.is_paused = true;
        let condition = self
            .breakpoints
            .get_breakpoint(function)
            .and_then(|bp| bp.condition.as_ref().map(|c| format!("{:?}", c)));

        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::BreakpointHit {
                function: function.to_string(),
                condition,
            },
            &mut plugin_ctx,
        );
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::ExecutionPaused {
                reason: "breakpoint".to_string(),
            },
            &mut plugin_ctx,
        );
    }

    /// Stage an execution so the debugger starts in a paused state without
    /// emitting a breakpoint log event.
    pub fn stage_execution(&mut self, function: &str, args: Option<&str>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_current_function(function.to_string(), args.map(str::to_string));
            state.call_stack_mut().clear();
            state.call_stack_mut().push(function.to_string(), None);
        }

        self.paused = true;
    }

    fn update_call_stack(&mut self, total_duration: std::time::Duration) -> Result<()> {
        let events = self.executor.get_diagnostic_events()?;

        let current_func = if let Ok(state) = self.state.lock() {
            state.current_function().unwrap_or("entry").to_string()
        } else {
            "entry".to_string()
        };

        if let Ok(mut state) = self.state.lock() {
            let stack = state.call_stack_mut();
            stack.clear();
            stack.push(current_func, None);

            for event in events {
                let event_str = format!("{:?}", event);
                if event_str.contains("ContractCall")
                    || (event_str.contains("call") && event.contract_id.is_some())
                {
                    let contract_id = event.contract_id.as_ref().map(|cid| format!("{:?}", cid));
                    stack.push("nested_call".to_string(), contract_id);
                } else if (event_str.contains("ContractReturn") || event_str.contains("return"))
                    && stack.get_stack().len() > 1
                {
                    stack.pop();
                }
            }

            if let Some(mut frame) = stack.pop() {
                frame.duration = Some(total_duration);
                stack.push_frame(frame);
            }
        }

        Ok(())
    }

    /// Step into next instruction.
    pub fn step_into(&mut self) -> Result<bool> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let stepped = if let Ok(mut state) = self.state.lock() {
            self.stepper.step_into(&mut state)
        } else {
            false
        };
        self.paused = stepped;
        Ok(stepped)
    }

    /// Step over function calls.
    pub fn step_over(&mut self) -> Result<bool> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let stepped = if let Ok(mut state) = self.state.lock() {
            self.stepper.step_over(&mut state)
        } else {
            false
        };
        self.paused = stepped;
        Ok(stepped)
    }

    pub fn step_over_source_line(&mut self) -> Result<StepOverResult> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let (paused, location) = if let (Ok(mut state), Some(source_map)) =
            (self.state.lock(), self.source_map.as_ref())
        {
            let advanced = self.stepper.step_over_source_line(&mut state, source_map);
            let loc = state
                .current_instruction()
                .and_then(|i| source_map.lookup(i.offset));
            (advanced, loc)
        } else {
            (false, None)
        };

        self.paused = paused;
        Ok(StepOverResult { paused, location })
    }

    /// Step out of current function.
    pub fn step_out(&mut self) -> Result<bool> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let stepped = if let Ok(mut state) = self.state.lock() {
            self.stepper.step_out(&mut state)
        } else {
            false
        };
        self.paused = stepped;
        Ok(stepped)
    }

    /// Step to next basic block.
    pub fn step_block(&mut self) -> Result<bool> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let stepped = if let Ok(mut state) = self.state.lock() {
            self.stepper.step_block(&mut state)
        } else {
            false
        };
        self.paused = stepped;
        Ok(stepped)
    }

    /// Step backwards to previous instruction.
    pub fn step_back(&mut self) -> Result<bool> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        let stepped = if let Ok(mut state) = self.state.lock() {
            self.stepper.step_back(&mut state)
        } else {
            false
        };
        self.paused = stepped;
        Ok(stepped)
    }

    /// Start instruction stepping with given mode.
    pub fn start_instruction_stepping(&mut self, mode: StepMode) -> Result<()> {
        if !self.instruction_debug_enabled {
            return Err(miette::miette!("Instruction debugging not enabled"));
        }

        if let Ok(mut state) = self.state.lock() {
            self.stepper.start(mode, &mut state);
            self.paused = true;
        }

        Ok(())
    }

    /// Continue execution until next breakpoint.
    pub fn continue_execution(&mut self) -> Result<()> {
        self.paused = false;
        if let Ok(mut state) = self.state.lock() {
            self.stepper.continue_execution(&mut state);
        }

        let mut plugin_ctx = EventContext::new();
        plugin_ctx.is_paused = false;
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::ExecutionResumed,
            &mut plugin_ctx,
        );
        Ok(())
    }

    fn pause_at_function(&mut self, function: &str, condition: Option<String>) {
        crate::logging::log_breakpoint(function);
        self.paused = true;

        if let Ok(mut state) = self.state.lock() {
            state.set_current_function(function.to_string(), None);
            state.call_stack().display();
        }

        let mut plugin_ctx = EventContext::new();
        plugin_ctx.is_paused = true;
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::BreakpointHit {
                function: function.to_string(),
                condition,
            },
            &mut plugin_ctx,
        );
        crate::plugin::registry::dispatch_global_event(
            &ExecutionEvent::ExecutionPaused {
                reason: "breakpoint".to_string(),
            },
            &mut plugin_ctx,
        );
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn state(&self) -> Arc<Mutex<DebugState>> {
        Arc::clone(&self.state)
    }

    pub fn current_instruction(&self) -> Option<Instruction> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.current_instruction().cloned())
    }

    pub fn get_instruction_context(&self, context_size: usize) -> Vec<(usize, Instruction, bool)> {
        if let Ok(state) = self.state.lock() {
            state
                .get_instruction_context(context_size)
                .into_iter()
                .map(|(idx, inst, current)| (idx, inst.clone(), current))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn breakpoints_mut(&mut self) -> &mut BreakpointManager {
        &mut self.breakpoints
    }

    pub fn breakpoints(&self) -> &BreakpointManager {
        &self.breakpoints
    }

    pub fn executor(&self) -> &ContractExecutor {
        &self.executor
    }

    pub fn executor_mut(&mut self) -> &mut ContractExecutor {
        &mut self.executor
    }

    /// Compatibility method for non-instruction stepping.
    pub fn step(&mut self) -> Result<()> {
        if self.instruction_debug_enabled {
            let _ = self.step_into()?;
        }
        if let Ok(mut state) = self.state.lock() {
            state.increment_step();
        }
        Ok(())
    }
}
