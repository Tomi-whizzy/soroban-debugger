use crate::debugger::instruction_pointer::StepMode;
use crate::runtime::instruction::Instruction;
use crossterm::style::Stylize;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Verbosity level stored as u8: 0 = Quiet, 1 = Normal, 2 = Verbose
static VERBOSITY_LEVEL: AtomicU8 = AtomicU8::new(1);

/// Pretty printing utilities for debugger output
pub struct Formatter;

impl Formatter {
    /// Format a value for display.
    pub fn format_value(value: &str) -> String {
        value.to_string()
    }

    /// Format storage key-value pair.
    pub fn format_storage_entry(key: &str, value: &str) -> String {
        format!("{} = {}", key, value)
    }

    /// Format a function call.
    pub fn format_function_call(name: &str, args: Option<&str>) -> String {
        if let Some(args) = args {
            format!("{}({})", name, args)
        } else {
            format!("{}()", name)
        }
    }

    /// Format budget information.
    pub fn format_budget(cpu: u64, cpu_limit: u64, mem: u64, mem_limit: u64) -> String {
        let cpu_pct = if cpu_limit == 0 {
            0.0
        } else {
            (cpu as f64 / cpu_limit as f64) * 100.0
        };
        let mem_pct = if mem_limit == 0 {
            0.0
        } else {
            (mem as f64 / mem_limit as f64) * 100.0
        };

        format!(
            "CPU: {}/{} ({:.1}%) | Memory: {}/{} bytes ({:.1}%)",
            cpu, cpu_limit, cpu_pct, mem, mem_limit, mem_pct
        )
    }

    /// Format a single instruction for display.
    pub fn format_instruction(instruction: &Instruction, is_current: bool) -> String {
        let prefix = if is_current { "►" } else { " " };
        let operands = instruction.operands();

        if operands.is_empty() {
            format!(
                "{} {:08x}: {}",
                prefix,
                instruction.offset,
                instruction.name()
            )
        } else {
            format!(
                "{} {:08x}: {} {}",
                prefix,
                instruction.offset,
                instruction.name(),
                operands
            )
        }
    }

    /// Format instruction context with surrounding instructions.
    pub fn format_instruction_context(
        context: &[(usize, Instruction, bool)],
        _context_size: usize,
    ) -> String {
        if context.is_empty() {
            return "No instructions available".to_string();
        }

        let mut lines = vec!["Instruction Context".to_string()];
        lines.extend(context.iter().map(|(idx, instruction, is_current)| {
            format!(
                "{:>4}: {}",
                idx,
                Self::format_instruction(instruction, *is_current)
            )
        }));
        lines.join("\n")
    }

    /// Format instruction pointer state.
    pub fn format_instruction_pointer_state(
        current_index: usize,
        call_depth: u32,
        step_mode: Option<StepMode>,
        is_stepping: bool,
    ) -> String {
        let mode = match step_mode {
            Some(StepMode::StepInto) => "Step Into",
            Some(StepMode::StepOver) => "Step Over",
            Some(StepMode::StepOut) => "Step Out",
            Some(StepMode::StepBlock) => "Step Block",
            None => "None",
        };

        format!(
            "Instruction Pointer\n  index: {}\n  call_depth: {}\n  step_mode: {}\n  stepping: {}",
            current_index,
            call_depth,
            mode,
            if is_stepping { "Active" } else { "Inactive" }
        )
    }

    /// Format instruction statistics.
    pub fn format_instruction_stats(
        total_instructions: usize,
        current_index: usize,
        instructions_executed: usize,
    ) -> String {
        let progress = if total_instructions == 0 {
            0.0
        } else {
            (current_index as f64 / total_instructions as f64) * 100.0
        };

        format!(
            "Execution Stats\n  total: {}\n  current: {}\n  executed: {}\n  progress: {:.1}%",
            total_instructions, current_index, instructions_executed, progress
        )
    }

    /// Format stepping help.
    pub fn format_stepping_help() -> String {
        [
            "Stepping commands:",
            "  n, next       Step to next instruction",
            "  s, step, into Step into calls",
            "  o, over       Step over calls",
            "  u, out        Step out of function",
            "  b, block      Step to next basic block",
            "  p, prev       Step back",
            "  c, continue   Continue execution",
            "  i, info       Show instruction state",
            "  ctx, context  Show instruction context",
            "  h, help       Show this help",
            "  q, quit       Exit stepping mode",
        ]
        .join("\n")
    }

    /// Format an informational message in blue.
    pub fn info(message: impl AsRef<str>) -> String {
        Self::apply_color(message.as_ref(), ColorKind::Info)
    }

    /// Format a success message in green.
    pub fn success(message: impl AsRef<str>) -> String {
        Self::apply_color(message.as_ref(), ColorKind::Success)
    }

    /// Format a warning message in yellow.
    pub fn warning(message: impl AsRef<str>) -> String {
        Self::apply_color(message.as_ref(), ColorKind::Warning)
    }

    /// Format an error message in red.
    pub fn error(message: impl AsRef<str>) -> String {
        Self::apply_color(message.as_ref(), ColorKind::Error)
    }

    /// Configure whether ANSI colors are enabled.
    pub fn configure_colors(enable: bool) {
        COLOR_ENABLED.store(enable, Ordering::Relaxed);
    }

    /// Auto-configure color output based on environment.
    pub fn configure_colors_from_env() {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        Self::configure_colors(!no_color);
    }

    /// Set the global verbosity level (0 = Quiet, 1 = Normal, 2 = Verbose).
    pub fn set_verbosity(level: u8) {
        VERBOSITY_LEVEL.store(level, Ordering::Relaxed);
    }

    /// Returns true when quiet mode is active (only errors and return values).
    pub fn is_quiet() -> bool {
        VERBOSITY_LEVEL.load(Ordering::Relaxed) == 0
    }

    /// Returns true when verbose mode is active (show all internal details).
    pub fn is_verbose() -> bool {
        VERBOSITY_LEVEL.load(Ordering::Relaxed) >= 2
    }

    fn apply_color(message: &str, kind: ColorKind) -> String {
        if !COLOR_ENABLED.load(Ordering::Relaxed) {
            return message.to_string();
        }

        match kind {
            ColorKind::Info => format!("{}", message.blue()),
            ColorKind::Success => format!("{}", message.green()),
            ColorKind::Warning => format!("{}", message.yellow()),
            ColorKind::Error => format!("{}", message.red()),
        }
    }

    pub fn format_compact_u64(value: u64) -> String {
        const K: u64 = 1_000;
        const M: u64 = 1_000_000;
        const B: u64 = 1_000_000_000;

        if value >= B {
            format!("{:.2}B", value as f64 / B as f64)
        } else if value >= M {
            format!("{:.2}M", value as f64 / M as f64)
        } else if value >= K {
            format!("{:.2}K", value as f64 / K as f64)
        } else {
            value.to_string()
        }
    }

    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Render a sparkline for a numeric series (downsampled to `width`).
    pub fn sparkline(values: &[u64], width: usize) -> String {
        if values.is_empty() || width == 0 {
            return String::new();
        }

        let buckets = width.min(values.len());
        let mut sampled: Vec<u64> = Vec::with_capacity(buckets);

        if values.len() <= buckets {
            sampled.extend_from_slice(values);
        } else {
            for i in 0..buckets {
                let start = i * values.len() / buckets;
                let end = ((i + 1) * values.len() / buckets).max(start + 1);
                let mut sum: u128 = 0;
                for v in &values[start..end] {
                    sum = sum.saturating_add(*v as u128);
                }
                sampled.push((sum / (end - start) as u128) as u64);
            }
        }

        let min = sampled.iter().copied().min().unwrap_or(0);
        let max = sampled.iter().copied().max().unwrap_or(min);

        let levels: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        if min == max {
            return std::iter::repeat_n(levels[0], sampled.len()).collect();
        }

        sampled
            .into_iter()
            .map(|v| {
                let ratio = (v.saturating_sub(min)) as f64 / (max - min) as f64;
                let idx = (ratio * (levels.len() - 1) as f64).round() as usize;
                levels[idx.min(levels.len() - 1)]
            })
            .collect()
    }
}

#[derive(Copy, Clone)]
enum ColorKind {
    Info,
    Success,
    Warning,
    Error,
}

static COLOR_ENABLED: AtomicBool = AtomicBool::new(true);
