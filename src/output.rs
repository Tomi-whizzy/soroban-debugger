//! Output and accessibility configuration for screen-reader compatible CLI.
//!
//! Supports `NO_COLOR` (disable ANSI colors) and `--no-unicode` (ASCII-only output).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

static NO_UNICODE: AtomicBool = AtomicBool::new(false);
static COLORS_ENABLED: AtomicBool = AtomicBool::new(true);
pub const SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputStatus {
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputError {
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginIncidentType {
    Panic,
    Timeout,
}

#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvocationReason {
    Entrypoint,
    CrossContract,
    Replay,
    Plugin,
}

impl InvocationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entrypoint => "entrypoint",
            Self::CrossContract => "cross_contract",
            Self::Replay => "replay",
            Self::Plugin => "plugin",
        }
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplayArtifactKind {
    Manifest,
    Trace,
    ContractWasm,
    NetworkSnapshot,
    StorageImport,
    StorageExport,
    OutputReport,
    GeneratedTest,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ReplayArtifactFile {
    pub kind: ReplayArtifactKind,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ReplayArtifactManifest {
    pub schema_version: String,
    pub artifact_group: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    pub files: Vec<ReplayArtifactFile>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PluginIncidentReport {
    pub plugin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_path: Option<String>,
    pub invocation_kind: String,
    pub incident: PluginIncidentType,
    pub action_taken: String,
    pub core_debugger_status: String,
    pub message: String,
}

impl PluginIncidentReport {
    pub fn summary_line(&self) -> String {
        format!(
            "Plugin incident: '{}' {} during {}. Action: {}. Core debugger status: {}.",
            self.plugin,
            match self.incident {
                PluginIncidentType::Panic => "panicked",
                PluginIncidentType::Timeout => "timed out",
            },
            self.invocation_kind,
            self.action_taken,
            self.core_debugger_status
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionedOutput<T>
where
    T: Serialize,
{
    pub schema_version: &'static str,
    pub command: String,
    pub status: OutputStatus,
    pub result: Option<T>,
    pub error: Option<OutputError>,
}

impl<T> VersionedOutput<T>
where
    T: Serialize,
{
    pub fn success(command: impl Into<String>, result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: command.into(),
            status: OutputStatus::Success,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(command: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: command.into(),
            status: OutputStatus::Error,
            result: None,
            error: Some(OutputError {
                message: message.into(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SymbolicReplayBundle {
    pub schema_version: u8,
    pub command: String,

    pub contract: ContractInfo,
    pub invocation: InvocationInfo,
    pub config: ReplayConfig,

    pub storage_seed: Option<StorageSeed>,
    pub metadata: Option<ReplayMetadata>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContractInfo {
    pub sha256: String,
    pub path_hint: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InvocationInfo {
    pub function: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplayConfig {
    pub seed: Option<u64>,
    pub max_paths: Option<usize>,
    pub max_input_combinations: Option<usize>,
    pub max_breadth: Option<usize>,
    pub max_depth: Option<usize>,
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StorageSeed {
    pub format: String,
    pub data: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReplayMetadata {
    pub paths_explored: usize,
    pub panics_found: usize,
}

/// Global output/accessibility configuration.
pub struct OutputConfig;

impl OutputConfig {
    /// Configure from CLI flags and environment.
    /// Call once at startup after parsing args.
    pub fn configure(no_unicode: bool) {
        NO_UNICODE.store(no_unicode, Ordering::Relaxed);
        // NO_COLOR: if set and not empty, disable ANSI colors
        let no_color = std::env::var("NO_COLOR")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        COLORS_ENABLED.store(!no_color, Ordering::Relaxed);
    }

    /// Whether `--no-unicode` is active (use ASCII-only output).
    #[inline]
    pub fn no_unicode() -> bool {
        NO_UNICODE.load(Ordering::Relaxed)
    }

    /// Whether ANSI colors are enabled (false when NO_COLOR is set and not empty).
    #[inline]
    pub fn colors_enabled() -> bool {
        COLORS_ENABLED.load(Ordering::Relaxed)
    }

    /// Replace box-drawing and other Unicode symbols with ASCII when `--no-unicode` is set.
    pub fn to_ascii(s: &str) -> String {
        if !Self::no_unicode() {
            return s.to_string();
        }
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            out.push(Self::replace_unicode_char(c));
        }
        out
    }

    fn replace_unicode_char(c: char) -> char {
        match c {
            // Corners
            '┌' | '┐' | '└' | '┘' => '+',
            // Horizontal
            '─' | '━' | '═' => '-',
            // Vertical
            '│' | '┃' => '|',
            // T-junctions
            '┬' | '┴' | '├' | '┤' | '┼' => '+',
            // Bullets / markers
            '•' => '*',
            '→' => '>',
            '⚠' => '!',
            '✔' | '✓' => '+',
            '✗' | '✘' => 'x',
            _ => c,
        }
    }

    /// Horizontal rule character(s) for section separators.
    pub fn rule_char() -> &'static str {
        "-"
    }

    /// Double-line rule character for headers.
    pub fn double_rule_char() -> &'static str {
        if Self::no_unicode() {
            "="
        } else {
            "\u{2550}" // ═
        }
    }

    /// A horizontal rule line (single line, for section separators).
    pub fn rule_line(len: usize) -> String {
        Self::rule_char().repeat(len)
    }

    /// A double horizontal rule line (for headers).
    pub fn double_rule_line(len: usize) -> String {
        Self::double_rule_char().repeat(len)
    }
}

pub fn format_resource_timeline(timeline: &[crate::inspector::budget::ResourceCheckpoint]) -> String {
    let mut out = String::new();
    use std::fmt::Write;
    
    writeln!(out, "| Timestamp (ms) | CPU Instructions | Memory Bytes | Location |").unwrap();
    writeln!(out, "|----------------|------------------|--------------|----------|").unwrap();
    
    for checkpoint in timeline {
        writeln!(
            out,
            "| {} | {} | {} | {} |",
            checkpoint.timestamp_ms,
            checkpoint.cpu_instructions,
            checkpoint.memory_bytes,
            checkpoint.location_name
        ).unwrap();
    }
    
    out
}

/// Status kind for text-equivalent labels (screen reader friendly).
#[derive(Clone, Copy)]
pub enum StatusLabel {
    Pass,
    Fail,
    Info,
    Warning,
    Error,
    Working,
}

impl StatusLabel {
    /// Text label to use when color is disabled or for accessibility.
    pub fn as_str(self) -> &'static str {
        match self {
            StatusLabel::Pass => "[PASS]",
            StatusLabel::Fail => "[FAIL]",
            StatusLabel::Info => "[INFO]",
            StatusLabel::Warning => "[WARN]",
            StatusLabel::Error => "[ERROR]",
            StatusLabel::Working => "[WORKING...]",
        }
    }
}

/// Spinner / progress: in no-unicode or accessibility mode, return static text instead of Unicode spinner.
pub fn spinner_text() -> &'static str {
    "[WORKING...]"
}

/// Helper for writing output to both stdout and optionally to a file
pub struct OutputWriter {
    file: Option<std::fs::File>,
}

impl OutputWriter {
    /// Create a new OutputWriter that optionally writes to a file
    pub fn new(path: Option<&std::path::Path>, append: bool) -> miette::Result<Self> {
        let file = if let Some(p) = path {
            if append {
                Some(
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(p)
                        .map_err(|e| miette::miette!("Failed to open output file: {}", e))?,
                )
            } else {
                Some(
                    std::fs::File::create(p)
                        .map_err(|e| miette::miette!("Failed to create output file: {}", e))?,
                )
            }
        } else {
            None
        };
        Ok(Self { file })
    }

    /// Write a line to the file (if configured)
    pub fn write(&mut self, text: &str) -> miette::Result<()> {
        if let Some(ref mut f) = self.file {
            use std::io::Write;
            writeln!(f, "{}", text)
                .map_err(|e| miette::miette!("Failed to write to output file: {}", e))?;
        }
        Ok(())
    }
}

/// Formats a resource timeline as a markdown table.
pub fn format_resource_timeline(
    timeline: &[crate::inspector::budget::ResourceCheckpoint],
) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "| Time (ms) | CPU Instructions | Memory Bytes | Location |"
    );
    let _ = writeln!(
        out,
        "|-----------|------------------|--------------|----------|"
    );
    for point in timeline {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            point.timestamp_ms, point.cpu_instructions, point.memory_bytes, point.location_name
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_bundle_serializes() {
        let bundle = SymbolicReplayBundle {
            schema_version: 1,
            command: "symbolic".to_string(),
            contract: ContractInfo {
                sha256: "abc".to_string(),
                path_hint: None,
            },
            invocation: InvocationInfo {
                function: "test".to_string(),
            },
            config: ReplayConfig {
                seed: Some(1),
                max_paths: None,
                max_input_combinations: None,
                max_breadth: None,
                max_depth: None,
                timeout_secs: None,
            },
            storage_seed: None,
            metadata: None,
        };

        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("schema_version"));
        assert!(json.contains("symbolic"));
    }

    #[test]
    fn test_replay_bundle_optional_fields_serialization() {
        let bundle = SymbolicReplayBundle {
            schema_version: 1,
            command: "symbolic".to_string(),
            contract: ContractInfo {
                sha256: "abc".to_string(),
                path_hint: Some("contract.wasm".to_string()),
            },
            invocation: InvocationInfo {
                function: "test".to_string(),
            },
            config: ReplayConfig {
                seed: None,
                max_paths: Some(10),
                max_input_combinations: Some(20),
                max_breadth: Some(2),
                max_depth: Some(3),
                timeout_secs: Some(30),
            },
            storage_seed: Some(StorageSeed {
                format: "json".to_string(),
                data: "{}".to_string(),
            }),
            metadata: Some(ReplayMetadata {
                paths_explored: 5,
                panics_found: 1,
            }),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("storage_seed"));
        assert!(json.contains("metadata"));
        assert!(json.contains("contract.wasm"));
    }
}
