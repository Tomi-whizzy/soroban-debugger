use super::events::{EventContext, ExecutionEvent};
use super::manifest::PluginManifest;
use std::any::Any;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur during plugin operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum PluginError {
    /// Plugin initialization failed
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    /// Plugin execution failed
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    /// Plugin not found
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// Invalid plugin
    #[error("Invalid plugin: {0}")]
    Invalid(String),

    /// Version mismatch
    #[error("Version mismatch: required {required}, found {found}")]
    VersionMismatch { required: String, found: String },

    /// Dependency error
    #[error("Dependency error: {0}")]
    DependencyError(String),
}

/// Custom CLI command that a plugin can provide
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Command name
    pub name: String,

    /// Command description
    pub description: String,

    /// Command arguments (name, description, required)
    pub arguments: Vec<(String, String, bool)>,
}

/// Custom output formatter that a plugin can provide
#[derive(Debug, Clone)]
pub struct OutputFormatter {
    /// Formatter name
    pub name: String,

    /// Supported output types
    pub supported_types: Vec<String>,
}

/// The main trait that all plugins must implement
///
/// This trait defines the interface for plugins to interact with the debugger.
/// Plugins can hook into execution events, provide custom CLI commands, and
/// add custom output formatters.
///
/// # Safety
///
/// This trait is designed to be implemented in dynamically loaded libraries.
/// All methods have default implementations to maintain backward compatibility
/// when the API evolves.
pub trait InspectorPlugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> PluginManifest;

    /// Initialize the plugin
    ///
    /// Called once when the plugin is loaded. Use this to set up any
    /// resources or state the plugin needs.
    fn initialize(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// Shutdown the plugin
    ///
    /// Called when the plugin is being unloaded. Use this to clean up
    /// any resources.
    fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }

    /// Handle an execution event
    ///
    /// Called whenever an execution event occurs. The plugin can inspect
    /// the event and context, and optionally modify the context for other
    /// plugins.
    fn on_event(&mut self, event: &ExecutionEvent, context: &mut EventContext) -> PluginResult<()> {
        let _ = (event, context); // Suppress unused warnings
        Ok(())
    }

    /// Get custom CLI commands provided by this plugin
    fn commands(&self) -> Vec<PluginCommand> {
        Vec::new()
    }

    /// Execute a custom CLI command
    ///
    /// Called when a user invokes one of the plugin's custom commands.
    /// The `args` parameter contains the command arguments as key-value pairs.
    fn execute_command(&mut self, command: &str, args: &[String]) -> PluginResult<String> {
        let _ = (command, args);
        Err(PluginError::ExecutionFailed(
            "Command not implemented".to_string(),
        ))
    }

    /// Get custom output formatters provided by this plugin
    fn formatters(&self) -> Vec<OutputFormatter> {
        Vec::new()
    }

    /// Format output using a custom formatter
    ///
    /// Called when a user requests output in a format provided by this plugin.
    fn format_output(&self, formatter: &str, data: &str) -> PluginResult<String> {
        let _ = (formatter, data);
        Err(PluginError::ExecutionFailed(
            "Formatter not implemented".to_string(),
        ))
    }

    /// Check if the plugin can be hot-reloaded
    fn supports_hot_reload(&self) -> bool {
        false
    }

    /// Prepare for hot-reload
    ///
    /// Called before the plugin is reloaded. The plugin should serialize
    /// any state it wants to preserve across reloads.
    fn prepare_reload(&self) -> PluginResult<Box<dyn Any + Send>> {
        Ok(Box::new(()))
    }

    /// Restore state after hot-reload
    ///
    /// Called after the plugin is reloaded. The plugin should restore
    /// any state from the previous version.
    fn restore_from_reload(&mut self, state: Box<dyn Any + Send>) -> PluginResult<()> {
        let _ = state;
        Ok(())
    }
}

/// Symbol name for the plugin constructor function
///
/// Every plugin shared library must export a function with this name
/// that returns a boxed instance of the plugin.
pub const PLUGIN_CONSTRUCTOR_SYMBOL: &str = "create_plugin";

/// Type of the plugin constructor function
pub type PluginConstructor = unsafe fn() -> *mut dyn InspectorPlugin;

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        manifest: PluginManifest,
    }

    impl InspectorPlugin for TestPlugin {
        fn metadata(&self) -> PluginManifest {
            self.manifest.clone()
        }
    }

    #[test]
    fn test_plugin_trait() {
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: "Test Author".to_string(),
            license: Some("MIT".to_string()),
            min_debugger_version: Some("0.1.0".to_string()),
            capabilities: super::super::manifest::PluginCapabilities {
                hooks_execution: true,
                provides_commands: false,
                provides_formatters: false,
                supports_hot_reload: false,
            },
            library: "test.so".to_string(),
            dependencies: vec![],
        };

        let plugin = TestPlugin {
            manifest: manifest.clone(),
        };

        assert_eq!(plugin.metadata().name, "test-plugin");
        assert_eq!(plugin.commands().len(), 0);
        assert_eq!(plugin.formatters().len(), 0);
    }
}
