use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct CommandOutput<T>
where
    T: Serialize,
{
    pub status: String,
    pub result: Option<T>,
    pub budget: Option<BudgetInfo>,
    pub errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct BudgetInfo {
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
}

pub fn write_json_pretty_file<T: Serialize>(path: &Path, value: &T) -> miette::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| miette::miette!("Failed to create parent directory: {}", e))?;
        }
    }

    let json =
        serde_json::to_string_pretty(value).map_err(|e| miette::miette!("JSON error: {}", e))?;
    std::fs::write(path, json).map_err(|e| miette::miette!("Failed to write file: {}", e))?;
    Ok(())
}
