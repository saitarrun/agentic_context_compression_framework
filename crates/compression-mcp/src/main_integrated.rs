/// Example: Using Integrated Compression Manager
///
/// This shows how to use all 4 phases in a unified, simple API.
/// This replaces main.rs when using the fully integrated system.

use compression_mcp::{IntegratedCompressionManager, IntegratedConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // Initialize integrated manager with all phases enabled
    let config = IntegratedConfig {
        auto_compress_enabled: true,
        compress_threshold: 1000,
        excluded_tools: vec![],
        enable_personalization: true,
        learn_from_history: true,
        enable_persistent_storage: true,
        storage_path: "./headroom.db".to_string(),
        ccr_retention_days: 30,
        enable_cache_alignment: true,
        enable_inter_turn_dedup: true,
        safety_level: "moderate".to_string(),
        verbose_logging: false,
    };

    let manager = IntegratedCompressionManager::new(config)?;

    // Health check: all phases operational
    let health = manager.health_check()?;
    println!("Health Status: {:?}", health);

    // Example 1: Simple compression (all phases work together)
    println!("\n=== Example 1: Simple Compression ===");
    let agent_id = "agent-42";
    let tool_name = "shell";
    let raw_output = r#"
        {
            "status": "ok",
            "error": null,
            "metadata": {},
            "result": "success",
            "timestamp": "2026-07-05T10:30:45Z",
            "retry_count": 3
        }
    "#;

    let result = manager.compress(agent_id, tool_name, raw_output)?;
    println!("Compressed: {} → {} ({:.2}x, {} tokens saved)",
        result.original_output.len(),
        result.compressed_output.len(),
        result.compression_ratio,
        result.tokens_saved
    );

    // Example 2: Retrieve original (Phase 1/4)
    println!("\n=== Example 2: Retrieve Original ===");
    let original = manager.retrieve(&result.output_id)?;
    println!("Retrieved {} bytes (byte-equal: {})",
        original.len(),
        original == result.original_output
    );

    // Example 3: Record task result for personalization (Phase 3)
    println!("\n=== Example 3: Personalization Learning ===");
    manager.record_task_result(agent_id, true, 0.95, result.tokens_saved)?;
    let strategy = manager.get_agent_strategy(agent_id)?;
    println!("Agent strategy: {}", strategy);

    // Example 4: Get metrics across all phases (Phase 2, 3, 4)
    println!("\n=== Example 4: Metrics & Analytics ===");
    let metrics = manager.get_metrics_snapshot();
    println!("{}", metrics);

    // Example 5: Export metrics in multiple formats
    println!("\n=== Example 5: Export Metrics ===");
    let prometheus = manager.export_metrics("prometheus")?;
    println!("Prometheus export:\n{}", prometheus);

    // Example 6: Identify top agents (Phase 3)
    println!("\n=== Example 6: Top Agents ===");
    let top = manager.get_top_agents(5)?;
    println!("Top agents: {:?}", top);

    // Example 7: Handler loop (Phase 2 - automatic hooks)
    println!("\n=== Example 7: Hook Handler Loop ===");
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut writer = tokio::io::BufWriter::new(stdout);
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    // Send server capabilities
    let server_info = json!({
        "type": "integrated_compression_server",
        "version": "1.0",
        "phases": {
            "phase_1": "Foundation - Manual compression with CCR",
            "phase_2": "Automatic hooks - Transparent compression",
            "phase_3": "Personalization - Per-agent adaptive strategies",
            "phase_4": "Multi-session - Persistent learning & optimization",
            "cache_alignment": "KV Cache alignment & non-deterministic token normalization",
            "inter_turn_dedup": "Observation deduplication across turns"
        }
    });

    writer.write_all(server_info.to_string().as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // Main request loop
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }

        let line_trimmed = line.trim();
        if line_trimmed.is_empty() {
            continue;
        }

        // Parse MCP request
        if let Ok(request_json) = serde_json::from_str::<serde_json::Value>(line_trimmed) {
            if let Some(tool) = request_json.get("tool").and_then(|t| t.as_str()) {
                let response = match tool {
                    "compress" => {
                        let agent_id = request_json
                            .get("agent_id")
                            .and_then(|a| a.as_str())
                            .unwrap_or("default");
                        let tool_name = request_json
                            .get("tool_name")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        let raw_output = request_json
                            .get("raw_output")
                            .and_then(|o| o.as_str())
                            .unwrap_or("");

                        match manager.compress(agent_id, tool_name, raw_output) {
                            Ok(result) => json!({
                                "status": "success",
                                "output_id": result.output_id,
                                "compressed_output": result.compressed_output,
                                "compression_ratio": result.compression_ratio,
                                "tokens_saved": result.tokens_saved,
                                "content_type": format!("{:?}", result.content_type),
                            }),
                            Err(e) => json!({
                                "status": "error",
                                "message": e,
                            }),
                        }
                    }
                    "retrieve" => {
                        let output_id = request_json
                            .get("output_id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("");

                        match manager.retrieve(output_id) {
                            Ok(original) => json!({
                                "status": "success",
                                "original_output": original,
                            }),
                            Err(e) => json!({
                                "status": "error",
                                "message": e,
                            }),
                        }
                    }
                    "search" => {
                        let output_id = request_json
                            .get("output_id")
                            .and_then(|i| i.as_str())
                            .unwrap_or("");
                        let query = request_json
                            .get("query")
                            .and_then(|q| q.as_str())
                            .unwrap_or("");
                        let max_results = request_json
                            .get("max_results")
                            .and_then(|m| m.as_u64())
                            .unwrap_or(5) as usize;

                        match manager.search(output_id, query, max_results) {
                            Ok(matches) => json!({
                                "status": "success",
                                "output_id": output_id,
                                "matches": matches,
                                "total_matches": matches.len()
                            }),
                            Err(e) => json!({
                                "status": "error",
                                "message": e,
                            }),
                        }
                    }
                    "stats" => json!({
                        "status": "success",
                        "metrics": manager.get_metrics_snapshot(),
                    }),
                    _ => json!({
                        "status": "error",
                        "message": format!("Unknown tool: {}", tool),
                    }),
                };

                writer.write_all(response.to_string().as_bytes()).await?;
                writer.write_all(b"\n").await?;
                writer.flush().await?;
            }
        }
    }

    Ok(())
}
