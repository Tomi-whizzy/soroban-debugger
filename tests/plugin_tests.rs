/// Tests for the plugin system
use soroban_debugger::plugin::{
    EventContext, ExecutionEvent, InspectorPlugin, PluginCapabilities, PluginCommand, PluginError,
    PluginLoader, PluginManifest, PluginRegistry, PluginResult,
};
use std::any::Any;
use tempfile::TempDir;

/// Mock plugin for testing
struct MockPlugin {
    manifest: PluginManifest,
    event_count: usize,
    initialized: bool,
}

impl MockPlugin {
    fn new(name: &str) -> Self {
        Self {
            manifest: PluginManifest {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: "Mock plugin for testing".to_string(),
                author: "Test".to_string(),
                license: Some("MIT".to_string()),
                min_debugger_version: Some("0.1.0".to_string()),
                capabilities: PluginCapabilities {
                    hooks_execution: true,
                    provides_commands: true,
                    provides_formatters: false,
                    supports_hot_reload: true,
                },
                library: "libmock.so".to_string(),
                dependencies: vec![],
                signature: None,
            },
            event_count: 0,
            initialized: false,
        }
    }
}

impl InspectorPlugin for MockPlugin {
    fn metadata(&self) -> PluginManifest {
        self.manifest.clone()
    }

    fn initialize(&mut self) -> PluginResult<()> {
        self.initialized = true;
        Ok(())
    }

    fn shutdown(&mut self) -> PluginResult<()> {
        self.initialized = false;
        Ok(())
    }

    fn on_event(
        &mut self,
        _event: &ExecutionEvent,
        _context: &mut EventContext,
    ) -> PluginResult<()> {
        self.event_count += 1;
        Ok(())
    }

    fn commands(&self) -> Vec<PluginCommand> {
        vec![PluginCommand {
            name: "test-command".to_string(),
            description: "A test command".to_string(),
            arguments: vec![],
        }]
    }

    fn execute_command(&mut self, command: &str, _args: &[String]) -> PluginResult<String> {
        match command {
            "test-command" => Ok("Command executed".to_string()),
            _ => Err(PluginError::ExecutionFailed("Unknown command".to_string())),
        }
    }

    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn prepare_reload(&self) -> PluginResult<Box<dyn Any + Send>> {
        Ok(Box::new(self.event_count))
    }

    fn restore_from_reload(&mut self, state: Box<dyn Any + Send>) -> PluginResult<()> {
        if let Ok(count) = state.downcast::<usize>() {
            self.event_count = *count;
            Ok(())
        } else {
            Err(PluginError::ExecutionFailed("Invalid state".to_string()))
        }
    }
}

#[test]
fn test_plugin_manifest_validation() {
    // Valid manifest
    let manifest = PluginManifest {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "Test plugin".to_string(),
        author: "Test Author".to_string(),
        license: Some("MIT".to_string()),
        min_debugger_version: Some("0.1.0".to_string()),
        capabilities: PluginCapabilities::default(),
        library: "test.so".to_string(),
        dependencies: vec![],
        signature: None,
    };

    assert!(manifest.validate().is_ok());

    // Invalid version
    let mut invalid_manifest = manifest.clone();
    invalid_manifest.version = "1.0".to_string();
    assert!(invalid_manifest.validate().is_err());

    // Empty name
    let mut invalid_manifest = manifest.clone();
    invalid_manifest.name = "".to_string();
    assert!(invalid_manifest.validate().is_err());
}

#[test]
fn test_mock_plugin_creation() {
    let mut plugin = MockPlugin::new("test");

    assert_eq!(plugin.metadata().name, "test");
    assert!(!plugin.initialized);

    // Initialize
    assert!(plugin.initialize().is_ok());
    assert!(plugin.initialized);

    // Shutdown
    assert!(plugin.shutdown().is_ok());
    assert!(!plugin.initialized);
}

#[test]
fn test_mock_plugin_events() {
    let mut plugin = MockPlugin::new("test");
    let mut context = EventContext::new();

    assert_eq!(plugin.event_count, 0);

    // Send some events
    let event = ExecutionEvent::BeforeFunctionCall {
        function: "test".to_string(),
        args: None,
    };

    assert!(plugin.on_event(&event, &mut context).is_ok());
    assert_eq!(plugin.event_count, 1);

    assert!(plugin.on_event(&event, &mut context).is_ok());
    assert_eq!(plugin.event_count, 2);
}

#[test]
fn test_mock_plugin_commands() {
    let mut plugin = MockPlugin::new("test");

    let commands = plugin.commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "test-command");

    // Execute valid command
    let result = plugin.execute_command("test-command", &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Command executed");

    // Execute invalid command
    let result = plugin.execute_command("invalid", &[]);
    assert!(result.is_err());
}

#[test]
fn test_mock_plugin_hot_reload() {
    let mut plugin = MockPlugin::new("test");
    let mut context = EventContext::new();

    // Process some events
    let event = ExecutionEvent::ExecutionResumed;
    for _ in 0..5 {
        plugin.on_event(&event, &mut context).unwrap();
    }
    assert_eq!(plugin.event_count, 5);

    // Prepare for reload
    let state = plugin.prepare_reload().unwrap();

    // Create new plugin and restore state
    let mut new_plugin = MockPlugin::new("test");
    assert_eq!(new_plugin.event_count, 0);

    assert!(new_plugin.restore_from_reload(state).is_ok());
    assert_eq!(new_plugin.event_count, 5);
}

#[test]
fn test_plugin_registry_creation() {
    let temp_dir = TempDir::new().unwrap();
    let registry = PluginRegistry::with_plugin_dir(temp_dir.path().to_path_buf());

    assert!(registry.is_ok());
    let registry = registry.unwrap();
    assert_eq!(registry.plugin_count(), 0);
}

#[test]
fn test_plugin_registry_statistics() {
    let temp_dir = TempDir::new().unwrap();
    let registry = PluginRegistry::with_plugin_dir(temp_dir.path().to_path_buf()).unwrap();

    let stats = registry.statistics();
    assert_eq!(stats.total, 0);
    assert_eq!(stats.hooks_execution, 0);
    assert_eq!(stats.provides_commands, 0);
}

#[test]
fn test_plugin_loader_default_dir() {
    let dir = PluginLoader::default_plugin_dir();
    assert!(dir.is_ok());

    let path = dir.unwrap();
    assert!(path.to_string_lossy().contains(".soroban-debug"));
    assert!(path.to_string_lossy().contains("plugins"));
}

#[test]
fn test_plugin_loader_discovery() {
    let temp_dir = TempDir::new().unwrap();
    let loader = PluginLoader::new(temp_dir.path().to_path_buf());

    // No plugins yet
    let manifests = loader.discover_plugins();
    assert_eq!(manifests.len(), 0);

    // Create a plugin directory with manifest
    let plugin_dir = temp_dir.path().join("test-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let manifest = PluginManifest {
        name: "test-plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "Test".to_string(),
        author: "Test".to_string(),
        license: Some("MIT".to_string()),
        min_debugger_version: Some("0.1.0".to_string()),
        capabilities: PluginCapabilities::default(),
        library: "test.so".to_string(),
        dependencies: vec![],
        signature: None,
    };

    let manifest_content = toml::to_string(&manifest).unwrap();
    std::fs::write(plugin_dir.join("plugin.toml"), manifest_content).unwrap();

    // Should discover the plugin
    let manifests = loader.discover_plugins();
    assert_eq!(manifests.len(), 1);
}

#[test]
fn test_event_context() {
    let mut context = EventContext::new();

    assert_eq!(context.stack_depth, 0);
    assert!(context.program_counter.is_none());
    assert!(!context.is_paused);
    assert!(context.custom_data.is_empty());

    // Modify context
    context.stack_depth = 5;
    context.program_counter = Some(42);
    context.is_paused = true;
    context
        .custom_data
        .insert("key".to_string(), "value".to_string());

    assert_eq!(context.stack_depth, 5);
    assert_eq!(context.program_counter, Some(42));
    assert!(context.is_paused);
    assert_eq!(context.custom_data.get("key"), Some(&"value".to_string()));
}

#[test]
fn test_execution_events() {
    use std::time::Duration;

    // Test different event variants
    let event1 = ExecutionEvent::BeforeFunctionCall {
        function: "test".to_string(),
        args: Some("[]".to_string()),
    };

    let event2 = ExecutionEvent::AfterFunctionCall {
        function: "test".to_string(),
        result: Ok("success".to_string()),
        duration: Duration::from_millis(100),
    };

    let event3 = ExecutionEvent::BreakpointHit {
        function: "test".to_string(),
        condition: Some("x > 10".to_string()),
    };

    let event4 = ExecutionEvent::Error {
        message: "Test error".to_string(),
        context: Some("Function: test".to_string()),
    };

    // Events should be created successfully
    let mut plugin = MockPlugin::new("test");
    let mut context = EventContext::new();

    assert!(plugin.on_event(&event1, &mut context).is_ok());
    assert!(plugin.on_event(&event2, &mut context).is_ok());
    assert!(plugin.on_event(&event3, &mut context).is_ok());
    assert!(plugin.on_event(&event4, &mut context).is_ok());

    assert_eq!(plugin.event_count, 4);
}

#[test]
fn test_plugin_capabilities() {
    let mut caps = PluginCapabilities::default();

    assert!(!caps.hooks_execution);
    assert!(!caps.provides_commands);
    assert!(!caps.provides_formatters);
    assert!(!caps.supports_hot_reload);

    // Modify capabilities
    caps.hooks_execution = true;
    caps.provides_commands = true;

    assert!(caps.hooks_execution);
    assert!(caps.provides_commands);
}

#[test]
fn test_plugin_error_types() {
    let err1 = PluginError::InitializationFailed("test".to_string());
    let err2 = PluginError::ExecutionFailed("test".to_string());
    let err3 = PluginError::NotFound("test".to_string());
    let err4 = PluginError::Invalid("test".to_string());
    let err5 = PluginError::VersionMismatch {
        required: "1.0.0".to_string(),
        found: "0.9.0".to_string(),
    };
    let err6 = PluginError::DependencyError("test".to_string());
    let err7 = PluginError::TrustViolation("test".to_string());

    // All errors should display properly
    assert!(err1.to_string().contains("initialization"));
    assert!(err2.to_string().contains("execution"));
    assert!(err3.to_string().contains("not found"));
    assert!(err4.to_string().contains("Invalid"));
    assert!(err5.to_string().contains("mismatch"));
    assert!(err6.to_string().contains("Dependency"));
    assert!(err7.to_string().contains("trust policy"));
}
