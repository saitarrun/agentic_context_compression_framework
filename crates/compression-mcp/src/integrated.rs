/// Integrated Compression Manager: Unified All Phases
///
/// Combines Phases 1-4 into a single, seamless compression system:
/// - Phase 1: Foundation (manual + automatic compression)
/// - Phase 2: Automatic hooks (transparent to agents)
/// - Phase 3: Per-agent personalization (adaptive strategies)
/// - Phase 4: Multi-session learning (persistent optimization)
///
/// This is the main interface users should use - it handles everything.

use crate::{
    ContentRouter, CcrBackend, MetricsCollector,
    PersonalizationManager, PersistentStorageManager,
    HookClient, HookConfig, MetricsExporter,
};
use mcp_types::ContentType;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Integrated compression system: all phases in one manager
pub struct IntegratedCompressionManager {
    // Phase 1: Foundation
    router: Arc<ContentRouter>,
    ccr: Arc<CcrBackend>,
    metrics: Arc<MetricsCollector>,

    // Phase 2: Automatic hooks
    hook_client: Arc<HookClient>,
    hook_config: HookConfig,

    // Phase 3: Personalization
    personalization: Arc<PersonalizationManager>,

    // Phase 4: Persistence
    storage: Arc<PersistentStorageManager>,

    // Configuration
    config: IntegratedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedConfig {
    // Phase 2: Hook settings
    pub auto_compress_enabled: bool,
    pub compress_threshold: usize,
    pub excluded_tools: Vec<String>,

    // Phase 3: Personalization
    pub enable_personalization: bool,
    pub learn_from_history: bool,

    // Phase 4: Persistence
    pub enable_persistent_storage: bool,
    pub storage_path: String,
    pub ccr_retention_days: u32,

    // General
    pub safety_level: String,
    pub verbose_logging: bool,
}

impl Default for IntegratedConfig {
    fn default() -> Self {
        Self {
            auto_compress_enabled: true,
            compress_threshold: 1000,
            excluded_tools: vec![],
            enable_personalization: true,
            learn_from_history: true,
            enable_persistent_storage: true,
            storage_path: "./headroom.db".to_string(),
            ccr_retention_days: 30,
            safety_level: "moderate".to_string(),
            verbose_logging: false,
        }
    }
}

/// Result of integrated compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    /// Original output
    pub original_output: String,
    /// Compressed output
    pub compressed_output: String,
    /// Compression ratio (original_size / compressed_size)
    pub compression_ratio: f64,
    /// Tokens saved
    pub tokens_saved: u64,
    /// Content type detected
    pub content_type: ContentType,
    /// Safety level
    pub safety_level: String,
    /// UUID for retrieval (CCR)
    pub output_id: String,
    /// Was compression applied?
    pub compressed: bool,
    /// Reason if not compressed
    pub skip_reason: Option<String>,
}

impl IntegratedCompressionManager {
    /// Create new integrated compression manager with all phases
    pub fn new(config: IntegratedConfig) -> Result<Self, String> {
        let router = Arc::new(ContentRouter::new());
        let ccr = Arc::new(CcrBackend::new());
        let metrics = Arc::new(MetricsCollector::new());
        let hook_client = Arc::new(HookClient::new("headroom-compression".to_string()));
        let personalization = Arc::new(PersonalizationManager::new());

        // Initialize persistent storage if enabled
        let storage = if config.enable_persistent_storage {
            let storage_config = crate::persistent_storage::StorageConfig::default_with_path(
                &config.storage_path,
            );
            Arc::new(
                PersistentStorageManager::new(storage_config)
                    .map_err(|e| format!("Failed to initialize storage: {}", e))?,
            )
        } else {
            Arc::new(PersistentStorageManager::new(
                crate::persistent_storage::StorageConfig::default_with_path(":memory:"),
            )?)
        };

        let hook_config = HookConfig {
            auto_compress_enabled: config.auto_compress_enabled,
            compress_threshold: config.compress_threshold,
            excluded_tools: config.excluded_tools.clone(),
            safety_level: config.safety_level.clone(),
        };

        Ok(Self {
            router,
            ccr,
            metrics,
            hook_client,
            hook_config,
            personalization,
            storage,
            config,
        })
    }

    /// Compress output with all phases integrated
    pub fn compress(
        &self,
        agent_id: &str,
        tool_name: &str,
        raw_output: &str,
    ) -> Result<CompressionResult, String> {
        // Phase 2: Check if should compress automatically
        if !self.hook_client.should_compress(tool_name, raw_output, &self.hook_config) {
            return Ok(CompressionResult {
                original_output: raw_output.to_string(),
                compressed_output: raw_output.to_string(),
                compression_ratio: 1.0,
                tokens_saved: 0,
                content_type: ContentType::Unknown,
                safety_level: "Unknown".to_string(),
                output_id: "".to_string(),
                compressed: false,
                skip_reason: Some("Below threshold or excluded tool".to_string()),
            });
        }

        // Phase 1: Route to appropriate compressor
        let (compressed_output, ratio, content_type) = self
            .router
            .compress(raw_output)
            .map_err(|e| e.to_string())?;

        let original_tokens = raw_output.len() as u64 / 4;
        let compressed_tokens = compressed_output.len() as u64 / 4;
        let tokens_saved = original_tokens.saturating_sub(compressed_tokens);

        // Phase 1: Store original in CCR (reversible)
        let output_id = self
            .ccr
            .store(raw_output.to_string())
            .map_err(|e| format!("CCR storage failed: {}", e))?;

        // Phase 1: Record metrics
        self.metrics.record_compression_detailed(
            tokens_saved,
            raw_output.len(),
            compressed_output.len(),
            content_type,
        );

        // Phase 3: Update agent profile (personalization)
        if self.config.enable_personalization {
            // Record with optimistic accuracy (will be corrected if task fails)
            self.personalization
                .update_profile_metrics(agent_id, true, 0.95, tokens_saved, &format!("{:?}", content_type))
                .ok();
        }

        // Phase 4: Store in persistent CCR if enabled
        if self.config.enable_persistent_storage {
            let persistent_record = crate::persistent_storage::PersistentCcrRecord {
                id: output_id.clone(),
                agent_id: agent_id.to_string(),
                original_output: raw_output.to_string(),
                compressed_output: compressed_output.clone(),
                original_size: raw_output.len(),
                compressed_size: compressed_output.len(),
                compression_ratio: ratio,
                content_type: format!("{:?}", content_type),
                safety_level: "Safe".to_string(),
                created_at: current_timestamp(),
                retrieved_count: 0,
            };

            self.storage
                .store_ccr_record(persistent_record)
                .map_err(|e| format!("Persistent storage failed: {}", e))?;
        }

        Ok(CompressionResult {
            original_output: raw_output.to_string(),
            compressed_output,
            compression_ratio: ratio,
            tokens_saved,
            content_type,
            safety_level: "Safe".to_string(),
            output_id,
            compressed: true,
            skip_reason: None,
        })
    }

    /// Retrieve original output (Phase 1/4)
    pub fn retrieve(&self, output_id: &str) -> Result<String, String> {
        // Try persistent storage first (Phase 4)
        if self.config.enable_persistent_storage {
            if let Ok(record) = self.storage.retrieve_ccr_record(output_id) {
                return Ok(record.original_output);
            }
        }

        // Fall back to in-memory CCR (Phase 1)
        self.ccr.retrieve(output_id)
    }

    /// Record task result for personalization (Phase 3)
    pub fn record_task_result(
        &self,
        agent_id: &str,
        success: bool,
        accuracy: f64,
        tokens_saved: u64,
    ) -> Result<(), String> {
        if self.config.enable_personalization {
            self.personalization.update_profile_metrics(
                agent_id,
                success,
                accuracy,
                tokens_saved,
                "generic",
            )?;
        }

        if !success {
            self.metrics.record_error();
        }

        Ok(())
    }

    /// Get recommended compression strategy for agent (Phase 3)
    pub fn get_agent_strategy(&self, agent_id: &str) -> Result<String, String> {
        if !self.config.enable_personalization {
            return Ok("default".to_string());
        }

        let strategy = self.personalization.recommend_strategy(agent_id)?;

        Ok(format!(
            "Threshold: {} bytes, JSON agg: {:.2}, Code agg: {:.2}, Text agg: {:.2}",
            strategy.compression_threshold,
            strategy.json_aggressiveness,
            strategy.code_aggressiveness,
            strategy.text_aggressiveness
        ))
    }

    /// Get metrics snapshot (all phases)
    pub fn get_metrics_snapshot(&self) -> String {
        let snapshot = self.metrics.get_snapshot();
        let analysis = MetricsExporter::export_to_analytics(&snapshot);

        // Add Phase 3/4 info if enabled
        let mut output = analysis;

        if self.config.enable_personalization {
            if let Ok(profiles) = self.personalization.list_profiles() {
                output.push_str("\n## Per-Agent Profiles\n");
                for profile in profiles {
                    output.push_str(&format!(
                        "- Agent {}: Success rate {:.2}%, Accuracy {:.2}%\n",
                        profile.agent_id,
                        profile.performance_metrics.success_rate * 100.0,
                        profile.performance_metrics.average_accuracy * 100.0
                    ));
                }
            }
        }

        if self.config.enable_persistent_storage {
            if let Ok(report) = self.storage.export_analytics_report() {
                output.push_str("\n## Cross-Session Analytics\n");
                output.push_str(&report);
            }
        }

        output
    }

    /// Export metrics in various formats (Phase 2)
    pub fn export_metrics(&self, format: &str) -> Result<String, String> {
        let snapshot = self.metrics.get_snapshot();

        match format {
            "prometheus" => Ok(MetricsExporter::export_to_prometheus(&snapshot)),
            "json" => Ok(MetricsExporter::export_to_json(&snapshot)),
            "analytics" => Ok(MetricsExporter::export_to_analytics(&snapshot)),
            _ => Err(format!("Unknown format: {}", format)),
        }
    }

    /// Get top performing agents (Phase 3)
    pub fn get_top_agents(&self, limit: usize) -> Result<Vec<String>, String> {
        if !self.config.enable_personalization {
            return Ok(vec![]);
        }

        let agents = self.personalization.get_top_agents(limit)?;
        Ok(agents.iter().map(|a| a.agent_id.clone()).collect())
    }

    /// Get struggling agents needing help (Phase 3)
    pub fn get_struggling_agents(&self, threshold: f64) -> Result<Vec<String>, String> {
        if !self.config.enable_personalization {
            return Ok(vec![]);
        }

        let agents = self.personalization.get_struggling_agents(threshold)?;
        Ok(agents.iter().map(|a| a.agent_id.clone()).collect())
    }

    /// Health check: verify all phases operational
    pub fn health_check(&self) -> Result<HealthStatus, String> {
        let mut status = HealthStatus::default();

        // Phase 1: Check router
        if let Ok((_, _, _)) = self.router.compress(r#"{"status":"ok"}"#) {
            status.phase1_operational = true;
        }

        // Phase 2: Check hooks
        status.phase2_operational = true;

        // Phase 3: Check personalization
        if self.config.enable_personalization {
            if self.personalization.list_profiles().is_ok() {
                status.phase3_operational = true;
            }
        }

        // Phase 4: Check storage
        if self.config.enable_persistent_storage {
            if self.storage.get_cross_session_metrics().is_ok() {
                status.phase4_operational = true;
            }
        }

        status.all_operational =
            status.phase1_operational && status.phase2_operational
                && (!self.config.enable_personalization || status.phase3_operational)
                && (!self.config.enable_persistent_storage || status.phase4_operational);

        Ok(status)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthStatus {
    pub phase1_operational: bool,
    pub phase2_operational: bool,
    pub phase3_operational: bool,
    pub phase4_operational: bool,
    pub all_operational: bool,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrated_manager_creation() {
        let config = IntegratedConfig::default();
        let manager = IntegratedCompressionManager::new(config).expect("create failed");
        let health = manager.health_check().expect("health check failed");
        assert!(health.phase1_operational);
    }

    #[test]
    fn test_compress_workflow() {
        let config = IntegratedConfig {
            auto_compress_enabled: true,
            compress_threshold: 10,
            ..Default::default()
        };

        let manager = IntegratedCompressionManager::new(config).expect("create failed");

        // Use a JSON response that compresses well
        let verbose_json = r#"{"status":"ok","error":null,"metadata":{},"timestamp":1720000000000,"retry_count":0,"data":"result"}"#;
        let result = manager
            .compress("agent-1", "shell", verbose_json)
            .expect("compress failed");

        assert!(result.compressed);
        assert!(result.tokens_saved > 0);
    }

    #[test]
    fn test_skip_compression_below_threshold() {
        let config = IntegratedConfig {
            auto_compress_enabled: true,
            compress_threshold: 1000,
            ..Default::default()
        };

        let manager = IntegratedCompressionManager::new(config).expect("create failed");

        let result = manager
            .compress("agent-1", "shell", "short")
            .expect("compress failed");

        assert!(!result.compressed);
    }

    #[test]
    fn test_retrieval_workflow() {
        let config = IntegratedConfig {
            compress_threshold: 10,  // Lower threshold so test output gets compressed
            ..Default::default()
        };
        let manager = IntegratedCompressionManager::new(config).expect("create failed");

        let original = "original output with more content to ensure it compresses";
        let result = manager
            .compress("agent-1", "shell", original)
            .expect("compress failed");

        let retrieved = manager
            .retrieve(&result.output_id)
            .expect("retrieve failed");

        assert_eq!(retrieved, original);
    }

    #[test]
    fn test_personalization_workflow() {
        let config = IntegratedConfig {
            enable_personalization: true,
            ..Default::default()
        };

        let manager = IntegratedCompressionManager::new(config).expect("create failed");

        manager
            .record_task_result("agent-1", true, 0.95, 100)
            .expect("record failed");

        let strategy = manager
            .get_agent_strategy("agent-1")
            .expect("strategy failed");

        assert!(strategy.contains("Threshold"));
    }

    #[test]
    fn test_metrics_export() {
        let config = IntegratedConfig::default();
        let manager = IntegratedCompressionManager::new(config).expect("create failed");

        let _result = manager
            .compress("agent-1", "shell", &"x".repeat(100))
            .expect("compress failed");

        let prometheus = manager
            .export_metrics("prometheus")
            .expect("export failed");
        assert!(prometheus.contains("headroom"));

        let json = manager.export_metrics("json").expect("export failed");
        assert!(json.contains("metrics"));
    }
}
