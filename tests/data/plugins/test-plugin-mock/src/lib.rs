use skylet_abi::prelude::*;

/// Mock test plugin for integration testing
#[skylet_plugin]
pub struct TestPluginMock;

#[skylet_plugin_impl]
impl Plugin for TestPluginMock {
    fn name(&self) -> &'static str {
        "test-plugin-mock"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn description(&self) -> &'static str {
        "Mock plugin for integration testing"
    }

    fn init(&mut self, _ctx: &mut PluginInitContext) -> Result<(), PluginError> {
        log::info!("Test plugin mock initialized");
        Ok(())
    }

    fn execute(&self, ctx: &PluginContext) -> Result<PluginResult, PluginError> {
        let input = ctx.input.as_str().unwrap_or("{}");
        log::info!("Test plugin mock executing with input: {}", input);

        // Parse input
        let data: serde_json::Value =
            serde_json::from_str(input).unwrap_or(serde_json::json!({"action": "unknown"}));

        // Handle different actions
        match data.get("action").and_then(|a| a.as_str()) {
            Some("echo") => {
                let result = serde_json::json!({
                    "status": "success",
                    "echo": data
                });
                Ok(PluginResult::Success(result.to_string()))
            }
            Some("fail") => Ok(PluginResult::Error(
                "Test plugin failure requested".to_string(),
            )),
            Some("slow") => {
                let delay = data.get("delay_ms").and_then(|d| d.as_u64()).unwrap_or(100);
                std::thread::sleep(std::time::Duration::from_millis(delay));
                Ok(PluginResult::Success(
                    "Slow operation completed".to_string(),
                ))
            }
            _ => Ok(PluginResult::Success("Test plugin executed".to_string())),
        }
    }

    fn cleanup(&mut self, _ctx: &PluginContext) -> Result<(), PluginError> {
        log::info!("Test plugin mock cleaned up");
        Ok(())
    }
}
