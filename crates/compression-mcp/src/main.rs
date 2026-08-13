use compression_mcp::{ContentRouter, ccr::CcrBackend, metrics::MetricsCollector, BpeEstimator};
use mcp_types::{
    CompressRequest, RetrieveRequest, SearchRequest, StatsRequest,
};
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let router = Arc::new(ContentRouter::new());
    let ccr = Arc::new(CcrBackend::new());
    let metrics = Arc::new(MetricsCollector::new());

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let mut reader = BufReader::new(stdin);
    let mut writer = BufWriter::new(stdout);

    tracing::info!("MCP Server started");

    // Write server capabilities
    let init_response = json!({
        "type": "server_info",
        "version": "0.2.0",
        "tools": [
            {
                "name": "headroom_compress",
                "description": "Compress tool output using content-aware algorithms",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Name of the tool that produced the output (shell, file, fetch, etc.)"
                        },
                        "raw_output": {
                            "type": "string",
                            "description": "Raw output from the tool to compress"
                        },
                        "task_goal": {
                            "type": "string",
                            "description": "Optional active agent goal to bias observation filtering"
                        },
                        "budget_level": {
                            "type": "string",
                            "description": "Optional budget level (light, balanced, aggressive, extreme)"
                        }
                    },
                    "required": ["tool_name", "raw_output"]
                }
            },
            {
                "name": "headroom_retrieve",
                "description": "Retrieve original output by ID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_id": {
                            "type": "string",
                            "description": "ID of the stored output"
                        }
                    },
                    "required": ["output_id"]
                }
            },
            {
                "name": "headroom_search",
                "description": "Search and retrieve specific relevant lines from a stored output by ID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "output_id": {
                            "type": "string",
                            "description": "ID of the stored output"
                        },
                        "query": {
                            "type": "string",
                            "description": "Keyword or diagnostic search query"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum matching lines to return (default: 5)"
                        }
                    },
                    "required": ["output_id", "query"]
                }
            },
            {
                "name": "headroom_stats",
                "description": "Get compression statistics",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "Optional session ID for filtering"
                        }
                    }
                }
            }
        ]
    });

    writer.write_all(init_response.to_string().as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Main server loop
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = handle_request(
            line,
            &router,
            &ccr,
            &metrics,
        )
        .await
        .unwrap_or_else(|e| json!({"error": e.to_string()}));

        writer.write_all(response.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

async fn handle_request(
    line: &str,
    router: &Arc<ContentRouter>,
    ccr: &Arc<CcrBackend>,
    metrics: &Arc<MetricsCollector>,
) -> Result<serde_json::Value, String> {
    let request: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    match request.get("method").and_then(|v| v.as_str()) {
        Some("headroom_compress") => {
            let params = request
                .get("params")
                .ok_or("Missing params")?
                .clone();
            let params: CompressRequest =
                serde_json::from_value(params).map_err(|e| e.to_string())?;

            let (compressed, ratio, content_type) = router
                .compress(&params.raw_output)
                .map_err(|e| e.to_string())?;

            let output_id = ccr
                .store(params.raw_output.clone())
                .map_err(|e| e.to_string())?;

            let original_tokens = BpeEstimator::count_tokens(&params.raw_output);
            let compressed_tokens = BpeEstimator::count_tokens(&compressed);
            let tokens_saved = original_tokens.saturating_sub(compressed_tokens);

            metrics.record_compression(tokens_saved);

            Ok(json!({
                "result": {
                    "output_id": output_id,
                    "compressed_output": compressed,
                    "compression_ratio": ratio,
                    "content_type": format!("{:?}", content_type),
                    "tokens_saved": tokens_saved
                }
            }))
        }
        Some("headroom_retrieve") => {
            let params = request
                .get("params")
                .ok_or("Missing params")?
                .clone();
            let params: RetrieveRequest =
                serde_json::from_value(params).map_err(|e| e.to_string())?;

            let original = ccr
                .retrieve(&params.output_id)
                .map_err(|e| e.to_string())?;

            Ok(json!({
                "result": {
                    "original_output": original
                }
            }))
        }
        Some("headroom_search") => {
            let params = request
                .get("params")
                .ok_or("Missing params")?
                .clone();
            let params: SearchRequest =
                serde_json::from_value(params).map_err(|e| e.to_string())?;

            let matches = ccr
                .search(&params.output_id, &params.query, params.max_results)
                .map_err(|e| e.to_string())?;

            Ok(json!({
                "result": {
                    "output_id": params.output_id,
                    "matches": matches,
                    "total_matches": matches.len()
                }
            }))
        }
        Some("headroom_stats") => {
            let snapshot = metrics.get_snapshot();

            Ok(json!({
                "result": {
                    "tokens_saved": snapshot.total_tokens_saved,
                    "compressions_count": snapshot.compressions_count,
                    "errors_count": snapshot.errors_count,
                    "average_accuracy": snapshot.average_accuracy
                }
            }))
        }
        _ => Err(format!("Unknown method: {:?}", request.get("method"))),
    }
}
