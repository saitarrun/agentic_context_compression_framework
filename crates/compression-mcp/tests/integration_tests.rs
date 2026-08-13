use compression_mcp::{
    CacheAligner, SmartCrusher, CodeCompressor, KompressBase, Compressor,
    CcrBackend, ContentRouter, IntegratedCompressionManager, IntegratedConfig,
    McpProxy, BpeEstimator,
};
use mcp_types::{BudgetLevel, ContentType};
use serde_json::json;

#[test]
fn test_cache_aligner_normalization() {
    let aligner = CacheAligner::new();
    let sample = "2026-08-13T17:15:18.123456Z [pid: 9912] segfault at 0x7ffee1b2c890 after 145ms with uuid 123e4567-e89b-12d3-a456-426614174000";
    let normalized = aligner.normalize(sample);

    assert!(normalized.contains("<TIMESTAMP>"));
    assert!(normalized.contains("<PID>"));
    assert!(normalized.contains("<HEX_ADDR>"));
    assert!(normalized.contains("<DURATION>"));
    assert!(normalized.contains("<UUID>"));
    assert!(!normalized.contains("0x7ffee1b2c890"));
    assert!(!normalized.contains("123e4567-e89b-12d3-a456-426614174000"));
}

#[test]
fn test_smart_crusher_tabular_projection_and_anomaly() {
    let crusher = SmartCrusher::new();

    let homogeneous_json = r#"[
        {"id": 1, "service": "auth", "status": "ok", "latency": "12ms"},
        {"id": 2, "service": "payment", "status": "ok", "latency": "14ms"},
        {"id": 3, "service": "database", "status": "error", "error": "connection refused"}
    ]"#;

    let (output, ratio) = crusher.compress(homogeneous_json).expect("compression failed");
    assert!(output.starts_with("#cols:"));
    assert!(output.contains("service"));
    assert!(output.contains("auth"));
    assert!(output.contains("payment"));
    assert!(output.contains("ANOMALY") || output.contains("connection refused"));
    assert!(ratio > 1.2);
}

#[test]
fn test_code_compressor_skeleton_and_diffs() {
    let compressor = CodeCompressor::new();

    // 1. Source code skeletonization
    let code_input = r#"
    pub fn process_records(records: &[Record]) -> Result<Summary, Error> {
        let mut total = 0;
        let mut count = 0;
        for record in records {
            total += record.amount;
            count += 1;
            log::debug!("Processing record {}", count);
        }
        Ok(Summary { total, count })
    }
    "#;

    let (compressed_code, code_ratio) = compressor.compress(code_input).expect("code compress failed");
    assert!(compressed_code.contains("pub fn process_records"));
    assert!(code_ratio >= 1.0);

    // 2. Diff compaction
    let diff_input = r#"
--- src/main.rs
+++ src/main.rs
@@ -10,6 +10,7 @@
 context line 1
 context line 2
-old_function();
+new_function();
 context line 3
 context line 4
    "#;

    let (compressed_diff, _) = compressor.compress(diff_input).expect("diff compress failed");
    assert!(compressed_diff.contains("+++ src/main.rs"));
    assert!(compressed_diff.contains("-old_function();"));
    assert!(compressed_diff.contains("+new_function();"));
}

#[test]
fn test_kompress_base_loop_and_progress_collapsing() {
    let compressor = KompressBase::new();

    let logs = r#"
    Starting database migration...
    [=>                  ] 5% (1/20)
    [=====>              ] 25% (5/20)
    [==========>         ] 50% (10/20)
    [===============>    ] 75% (15/20)
    [====================] 100% (20/20)
    Waiting for lock...
    Waiting for lock...
    Waiting for lock...
    Migration completed successfully.
    "#;

    let (compressed, ratio) = compressor.compress(logs).expect("text compress failed");
    assert!(compressed.contains("Starting database migration..."));
    assert!(compressed.contains("progress lines omitted") || !compressed.contains("[=>"));
    assert!(compressed.contains("repeated") || !compressed.contains("Waiting for lock...\nWaiting for lock...\nWaiting for lock..."));
    assert!(compressed.contains("Migration completed successfully."));
    assert!(ratio > 1.3);
}

#[test]
fn test_sub_observation_bm25_search() {
    let ccr = CcrBackend::new();
    let verbose_log = r#"
    2026-08-13 10:00:01 INFO Server boot
    2026-08-13 10:00:02 INFO Connecting to PostgreSQL at localhost:5432
    2026-08-13 10:00:05 ERROR Database connection timeout after 3000ms
    2026-08-13 10:00:06 FATAL Worker pool terminated unexpectedly with exit code 1
    2026-08-13 10:00:07 INFO Cleanup complete
    "#;

    let id = ccr.store(verbose_log.to_string()).expect("store failed");

    // Search specifically for the error
    let results = ccr.search(&id, "timeout Database", 3).expect("search failed");
    assert!(!results.is_empty());
    assert_eq!(results[0].line_number, 4);
    assert!(results[0].content.contains("Database connection timeout"));
}

#[test]
fn test_inter_turn_observation_deduplication() {
    let config = IntegratedConfig {
        compress_threshold: 10,
        enable_inter_turn_dedup: true,
        ..Default::default()
    };
    let manager = IntegratedCompressionManager::new(config).expect("create manager");

    let repeated_output = "Unchanged git status branch main, working directory clean, nothing to commit\n".repeat(5);

    // Turn 1
    let res1 = manager.compress("agent-test", "git_status", &repeated_output).expect("turn 1");
    assert!(res1.compressed);

    // Turn 2: identical output
    let res2 = manager.compress("agent-test", "git_status", &repeated_output).expect("turn 2");
    assert!(res2.compressed);
    assert!(res2.compressed_output.contains("identical to Turn #1"));
    assert!(res2.compression_ratio > 3.0);
}

#[test]
fn test_goal_conditioned_pruning() {
    let config = IntegratedConfig {
        compress_threshold: 10,
        ..Default::default()
    };
    let manager = IntegratedCompressionManager::new(config).expect("create manager");

    let build_output = r#"
    Compiling package-a v0.1.0
    Compiling package-b v0.1.0
    Compiling auth-service v0.2.1
    Compiling frontend v1.0.0
    error[E0425]: cannot find value `jwt_secret` in this scope in auth-service/src/token.rs:88
    Compiling utils v0.1.0
    Finished dev target(s)
    "#;

    let res = manager.compress_with_options(
        "agent-1",
        "cargo_build",
        build_output,
        Some("Fix jwt_secret in auth-service"),
        BudgetLevel::Aggressive,
    ).expect("compress with goal failed");

    assert!(res.compressed);
    assert!(res.compressed_output.contains("jwt_secret") || res.compressed_output.contains("auth-service"));
}

#[test]
fn test_bpe_token_estimator() {
    let short_text = "Hello world";
    let code_snippet = "async fn handle_connection(stream: TcpStream) -> Result<()> { Ok(()) }";

    let tokens_short = BpeEstimator::count_tokens(short_text);
    let tokens_code = BpeEstimator::count_tokens(code_snippet);

    assert!(tokens_short >= 2);
    assert!(tokens_code >= 10);
}

#[test]
fn test_transparent_mcp_proxy_interception() {
    let config = IntegratedConfig {
        compress_threshold: 10,
        ..Default::default()
    };
    let proxy = McpProxy::new(config).expect("create proxy");

    // 1. Intercept tools/list
    let list_json = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [{"name": "executeCommand", "description": "Run shell command"}]
        }
    }).to_string();

    let list_processed = proxy.process_message(&list_json, "agent-1");
    assert!(list_processed.contains("headroom_retrieve"));
    assert!(list_processed.contains("headroom_search"));

    // 2. Intercept tools/call
    let call_json = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": {
            "content": [
                {
                    "type": "text",
                    "text": "{\n  \"status\": \"ok\",\n  \"data\": \"test payload repeating to exceed threshold \".repeat(10)\n}"
                }
            ]
        }
    }).to_string();

    let call_processed = proxy.process_message(&call_json, "agent-1");
    assert!(call_processed.contains("headroom_retrieve") || call_processed.contains("Compressed"));
}
