use std::sync::Arc;
use crate::integrated::{IntegratedCompressionManager, IntegratedConfig};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;

/// Transparent MCP Reverse Proxy
///
/// Wraps any downstream MCP server (e.g. filesystem, postgres, github, bash)
/// and automatically intercepts and compresses `tools/call` outputs *in-flight*
/// before returning them to Claude/client, with zero agent prompt overhead.
pub struct McpProxy {
    manager: Arc<IntegratedCompressionManager>,
}

impl McpProxy {
    pub fn new(config: IntegratedConfig) -> Result<Self, String> {
        let manager = Arc::new(IntegratedCompressionManager::new(config)?);
        Ok(Self { manager })
    }

    /// Process a JSON-RPC message passing through the proxy
    pub fn process_message(&self, message_str: &str, agent_id: &str) -> String {
        let mut value: Value = match serde_json::from_str(message_str) {
            Ok(v) => v,
            Err(_) => return message_str.to_string(),
        };

        // 1. Intercept tools/list response to inject headroom_retrieve and headroom_search
        if let Some(result) = value.get_mut("result") {
            if let Some(tools) = result.get_mut("tools").and_then(|t| t.as_array_mut()) {
                tools.push(json!({
                    "name": "headroom_retrieve",
                    "description": "Retrieve the uncompressed original output for a given output_id",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "output_id": {
                                "type": "string",
                                "description": "The unique ID of the stored output"
                            }
                        },
                        "required": ["output_id"]
                    }
                }));

                tools.push(json!({
                    "name": "headroom_search",
                    "description": "Search for specific keywords/snippets inside a compressed output by ID",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "output_id": {
                                "type": "string",
                                "description": "The unique ID of the stored output"
                            },
                            "query": {
                                "type": "string",
                                "description": "Search query terms"
                            },
                            "max_results": {
                                "type": "integer",
                                "description": "Maximum matching lines to return (default: 5)"
                            }
                        },
                        "required": ["output_id", "query"]
                    }
                }));
            }

            // 2. Intercept tools/call response to automatically compress output in-flight
            if let Some(content) = result.get_mut("content").and_then(|c| c.as_array_mut()) {
                for item in content.iter_mut() {
                    if let Some(text_val) = item.get("text").and_then(|t| t.as_str()) {
                        if text_val.len() > 200 {
                            if let Ok(res) = self.manager.compress(agent_id, "proxy_tool", text_val) {
                                if res.compressed {
                                    let augmented = format!(
                                        "{}\n\n[Compressed: {:.1}x, {} tokens saved | ID: {} (use headroom_retrieve or headroom_search if full raw is needed)]",
                                        res.compressed_output,
                                        res.compression_ratio,
                                        res.tokens_saved,
                                        res.output_id
                                    );
                                    item["text"] = Value::String(augmented);
                                }
                            }
                        }
                    }
                }
            }
        }

        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_proxy_intercepts_tools_list() {
        let proxy = McpProxy::new(IntegratedConfig::default()).expect("create proxy");
        let list_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [
                    {
                        "name": "readFile",
                        "description": "Read file content"
                    }
                ]
            }
        }).to_string();

        let processed = proxy.process_message(&list_response, "agent-1");
        assert!(processed.contains("headroom_retrieve"));
        assert!(processed.contains("headroom_search"));
    }

    #[test]
    fn test_mcp_proxy_compresses_tool_call_output() {
        let config = IntegratedConfig {
            compress_threshold: 10,
            ..Default::default()
        };
        let proxy = McpProxy::new(config).expect("create proxy");

        let call_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": r#"{"status":"ok","timestamp":1720000000,"data":"large result ".repeat(50)}"#
                    }
                ]
            }
        }).to_string();

        let processed = proxy.process_message(&call_response, "agent-1");
        assert!(processed.contains("headroom_retrieve"));
    }
}
