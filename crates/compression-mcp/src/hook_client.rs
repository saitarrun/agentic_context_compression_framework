/// Hook client for Claude Code integration
/// Enables automatic compression via after_tool_response hook
///
/// This module handles communication between Claude Code's hook system
/// and the MCP compression server.

use serde_json::json;

/// Hook client for automatic compression
pub struct HookClient {
    mcp_server_name: String,
    timeout_ms: u64,
}

impl HookClient {
    /// Create a new hook client
    pub fn new(mcp_server_name: String) -> Self {
        Self {
            mcp_server_name,
            timeout_ms: 5000,
        }
    }

    /// Determine if output should be compressed
    pub fn should_compress(&self, tool_name: &str, output: &str, config: &HookConfig) -> bool {
        // Check if compression disabled globally
        if !config.auto_compress_enabled {
            return false;
        }

        // Check size threshold
        if output.len() < config.compress_threshold {
            return false;
        }

        // Check if tool is excluded
        if config.excluded_tools.contains(&tool_name.to_string()) {
            return false;
        }

        // Don't compress if output contains auth patterns
        if self.contains_auth_patterns(output) {
            return false;
        }

        true
    }

    /// Check if output contains authentication patterns
    fn contains_auth_patterns(&self, output: &str) -> bool {
        let lower = output.to_lowercase();
        [
            "authorization:",
            "api-key:",
            "bearer ",
            "secret=",
            "password:",
            "apikey:",
            "access_token:",
            "refresh_token:",
        ]
        .iter()
        .any(|pattern| lower.contains(pattern))
    }

    /// Call MCP server to compress output (stub for hook integration)
    pub fn compress_output(
        &self,
        tool_name: &str,
        output: &str,
    ) -> Result<HookCompressionResult, String> {
        // In production, this would:
        // 1. Connect to MCP server via socket/TCP
        // 2. Send JSON-RPC request with headroom_compress method
        // 3. Wait for response with timeout
        // 4. Parse and validate response
        // 5. Return compressed output + metadata

        // For now, stub that indicates hook would call MCP server
        Ok(HookCompressionResult {
            tool_name: tool_name.to_string(),
            compressed_output: output.to_string(),
            compression_ratio: 1.0,
            tokens_saved: 0,
            output_id: None,
            safety_level: "Safe".to_string(),
            hook_metadata: json!({
                "hook_executed": true,
                "mcp_server": self.mcp_server_name,
                "method": "headroom_compress",
                "status": "stub_for_implementation"
            }),
        })
    }

    /// Get hook configuration from environment
    pub fn get_config_from_env() -> HookConfig {
        HookConfig {
            auto_compress_enabled: std::env::var("HEADROOM_AUTO_COMPRESS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(true),
            compress_threshold: std::env::var("HEADROOM_COMPRESS_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            excluded_tools: std::env::var("HEADROOM_EXCLUDE_TOOLS")
                .map(|v| v.split(',').map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            safety_level: std::env::var("HEADROOM_SAFETY_LEVEL")
                .unwrap_or_else(|_| "moderate".to_string()),
        }
    }
}

impl Default for HookClient {
    fn default() -> Self {
        Self::new("headroom-compression".to_string())
    }
}

/// Hook configuration from Claude Code settings
#[derive(Debug, Clone)]
pub struct HookConfig {
    pub auto_compress_enabled: bool,
    pub compress_threshold: usize,
    pub excluded_tools: Vec<String>,
    pub safety_level: String,
}

/// Result of compression via hook
#[derive(Debug, Clone)]
pub struct HookCompressionResult {
    pub tool_name: String,
    pub compressed_output: String,
    pub compression_ratio: f64,
    pub tokens_saved: u64,
    pub output_id: Option<String>,
    pub safety_level: String,
    pub hook_metadata: serde_json::Value,
}

/// Hook response for Claude Code
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HookResponse {
    pub output: String,
    pub compression_metadata: Option<serde_json::Value>,
    pub compression_error: Option<String>,
    pub compression_skipped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_client_creation() {
        let client = HookClient::new("headroom-compression".to_string());
        assert_eq!(client.mcp_server_name, "headroom-compression");
        assert_eq!(client.timeout_ms, 5000);
    }

    #[test]
    fn test_hook_client_default() {
        let client = HookClient::default();
        assert_eq!(client.mcp_server_name, "headroom-compression");
    }

    #[test]
    fn test_should_compress_below_threshold() {
        let client = HookClient::new("test".to_string());
        let config = HookConfig {
            auto_compress_enabled: true,
            compress_threshold: 1000,
            excluded_tools: vec![],
            safety_level: "moderate".to_string(),
        };

        // Small output
        assert!(!client.should_compress("shell", "small", &config));
    }

    #[test]
    fn test_should_compress_above_threshold() {
        let client = HookClient::new("test".to_string());
        let config = HookConfig {
            auto_compress_enabled: true,
            compress_threshold: 100,
            excluded_tools: vec![],
            safety_level: "moderate".to_string(),
        };

        let large_output = "x".repeat(200);
        assert!(client.should_compress("shell", &large_output, &config));
    }

    #[test]
    fn test_should_not_compress_disabled() {
        let client = HookClient::new("test".to_string());
        let config = HookConfig {
            auto_compress_enabled: false,
            compress_threshold: 100,
            excluded_tools: vec![],
            safety_level: "moderate".to_string(),
        };

        let large_output = "x".repeat(200);
        assert!(!client.should_compress("shell", &large_output, &config));
    }

    #[test]
    fn test_should_not_compress_excluded_tool() {
        let client = HookClient::new("test".to_string());
        let config = HookConfig {
            auto_compress_enabled: true,
            compress_threshold: 100,
            excluded_tools: vec!["shell".to_string()],
            safety_level: "moderate".to_string(),
        };

        let large_output = "x".repeat(200);
        assert!(!client.should_compress("shell", &large_output, &config));
    }

    #[test]
    fn test_contains_auth_patterns() {
        let client = HookClient::default();

        assert!(client.contains_auth_patterns("Authorization: Bearer token123"));
        assert!(client.contains_auth_patterns("api-key: sk-1234567890"));
        assert!(client.contains_auth_patterns("Secret=mypassword"));
        assert!(!client.contains_auth_patterns("Response: success"));
    }

    #[test]
    fn test_should_not_compress_auth_data() {
        let client = HookClient::new("test".to_string());
        let config = HookConfig {
            auto_compress_enabled: true,
            compress_threshold: 10,
            excluded_tools: vec![],
            safety_level: "moderate".to_string(),
        };

        let auth_output = "Authorization: Bearer token123_this_is_a_secret_token";
        assert!(!client.should_compress("fetch", auth_output, &config));
    }

    #[test]
    fn test_hook_config_from_env() {
        std::env::set_var("HEADROOM_AUTO_COMPRESS", "true");
        std::env::set_var("HEADROOM_COMPRESS_THRESHOLD", "2000");
        std::env::set_var("HEADROOM_EXCLUDE_TOOLS", "ssh,sudo");

        let config = HookClient::get_config_from_env();
        assert!(config.auto_compress_enabled);
        assert_eq!(config.compress_threshold, 2000);
        assert_eq!(config.excluded_tools, vec!["ssh", "sudo"]);
    }
}
