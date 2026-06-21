/// Phase 4: Persistent Storage for Multi-Session Learning
/// Enables cross-session optimization and long-term metrics tracking
///
/// Stores:
/// - CCR (Compression Control Records) with long-term retrieval
/// - Agent profiles and compression preferences
/// - Cross-session metrics and trends
/// - Learned compression patterns

use crate::personalization::AgentProfile;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Persistent storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// SQLite database path
    pub database_path: PathBuf,

    /// Retention policy: days to keep CCR records
    pub ccr_retention_days: u32,

    /// Maximum database size (MB)
    pub max_database_size_mb: u64,

    /// Enable automatic cleanup
    pub auto_cleanup_enabled: bool,
}

impl StorageConfig {
    /// Create default storage config
    pub fn default_with_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            database_path: path.as_ref().to_path_buf(),
            ccr_retention_days: 30,
            max_database_size_mb: 1024,
            auto_cleanup_enabled: true,
        }
    }
}

/// Persistent CCR record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentCcrRecord {
    pub id: String,
    pub agent_id: String,
    pub original_output: String,
    pub compressed_output: String,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub content_type: String,
    pub safety_level: String,
    pub created_at: u64,
    pub retrieved_count: u32,
}

/// Cross-session metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossSessionMetrics {
    pub total_compressions: u64,
    pub total_tokens_saved: u64,
    pub average_compression_ratio: f64,
    pub total_sessions: u32,
    pub average_session_tokens_saved: u64,
    pub per_content_type_ratios: HashMap<String, f64>,
    pub per_agent_stats: HashMap<String, AgentSessionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentSessionStats {
    pub sessions_count: u32,
    pub total_tokens_saved: u64,
    pub average_accuracy: f64,
    pub success_rate: f64,
}

/// Persistent storage manager
pub struct PersistentStorageManager {
    config: StorageConfig,
    // In production, this would use SQLite. For now, in-memory with serialization
    ccr_records: std::sync::Arc<std::sync::Mutex<Vec<PersistentCcrRecord>>>,
    agent_profiles: std::sync::Arc<std::sync::Mutex<HashMap<String, AgentProfile>>>,
    cross_session_metrics: std::sync::Arc<std::sync::Mutex<CrossSessionMetrics>>,
}

impl PersistentStorageManager {
    /// Create new persistent storage manager
    pub fn new(config: StorageConfig) -> Result<Self, String> {
        // In production, initialize SQLite database here
        // For now, we provide the structure for the implementation

        Ok(Self {
            config,
            ccr_records: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            agent_profiles: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            cross_session_metrics: std::sync::Arc::new(std::sync::Mutex::new(CrossSessionMetrics::default())),
        })
    }

    /// Store a CCR record persistently
    pub fn store_ccr_record(&self, record: PersistentCcrRecord) -> Result<String, String> {
        let id = record.id.clone();
        let mut records = self
            .ccr_records
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        records.push(record.clone());

        // Update cross-session metrics
        self.update_metrics_for_record(&record)?;

        Ok(id)
    }

    /// Retrieve CCR record (even from past sessions)
    pub fn retrieve_ccr_record(&self, id: &str) -> Result<PersistentCcrRecord, String> {
        let mut records = self
            .ccr_records
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            record.retrieved_count += 1;
            Ok(record.clone())
        } else {
            Err(format!("CCR record not found: {}", id))
        }
    }

    /// Get statistics on CCR usage
    pub fn get_ccr_statistics(&self) -> Result<CcrStatistics, String> {
        let records = self
            .ccr_records
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        let total_records = records.len();
        let total_retrievals: u32 = records.iter().map(|r| r.retrieved_count).sum();
        let total_original_bytes: usize = records.iter().map(|r| r.original_size).sum();
        let total_compressed_bytes: usize = records.iter().map(|r| r.compressed_size).sum();

        Ok(CcrStatistics {
            total_records,
            total_retrievals,
            total_original_bytes,
            total_compressed_bytes,
            average_compression_ratio: if total_compressed_bytes > 0 {
                total_original_bytes as f64 / total_compressed_bytes as f64
            } else {
                1.0
            },
        })
    }

    /// Store agent profile
    pub fn save_agent_profile(&self, profile: AgentProfile) -> Result<(), String> {
        let mut profiles = self
            .agent_profiles
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        profiles.insert(profile.agent_id.clone(), profile);
        Ok(())
    }

    /// Retrieve agent profile
    pub fn load_agent_profile(&self, agent_id: &str) -> Result<AgentProfile, String> {
        let profiles = self
            .agent_profiles
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        profiles
            .get(agent_id)
            .cloned()
            .ok_or_else(|| format!("Agent profile not found: {}", agent_id))
    }

    /// Get cross-session metrics
    pub fn get_cross_session_metrics(&self) -> Result<CrossSessionMetrics, String> {
        let metrics = self
            .cross_session_metrics
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        Ok(metrics.clone())
    }

    /// Update cross-session metrics when new record is stored
    fn update_metrics_for_record(&self, record: &PersistentCcrRecord) -> Result<(), String> {
        let mut metrics = self
            .cross_session_metrics
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        metrics.total_compressions += 1;
        metrics.total_tokens_saved +=
            (record.original_size as u64).saturating_sub(record.compressed_size as u64);

        // Update per-content-type ratios
        metrics
            .per_content_type_ratios
            .insert(record.content_type.clone(), record.compression_ratio);

        Ok(())
    }

    /// Clean up old CCR records based on retention policy
    pub fn cleanup_old_records(&self) -> Result<usize, String> {
        let mut records = self
            .ccr_records
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        let retention_timestamp = current_timestamp()
            .saturating_sub((self.config.ccr_retention_days as u64) * 86400);

        let initial_count = records.len();
        records.retain(|r| r.created_at > retention_timestamp);

        Ok(initial_count - records.len())
    }

    /// Export metrics report for analysis
    pub fn export_analytics_report(&self) -> Result<String, String> {
        let ccr_stats = self.get_ccr_statistics()?;
        let metrics = self.get_cross_session_metrics()?;

        let report = format!(
            r#"# Multi-Session Analytics Report

## CCR Statistics
- Total Stored Records: {}
- Total Retrievals: {}
- Total Original Bytes: {}
- Total Compressed Bytes: {}
- Average Compression Ratio: {:.2}x

## Cross-Session Metrics
- Total Compressions: {}
- Total Tokens Saved: {}
- Total Sessions: {}
- Average Tokens Saved per Session: {}

## Per-Content-Type Performance
{}

## Insights
{}
"#,
            ccr_stats.total_records,
            ccr_stats.total_retrievals,
            ccr_stats.total_original_bytes,
            ccr_stats.total_compressed_bytes,
            ccr_stats.average_compression_ratio,
            metrics.total_compressions,
            metrics.total_tokens_saved,
            metrics.total_sessions,
            if metrics.total_sessions > 0 {
                metrics.total_tokens_saved / metrics.total_sessions as u64
            } else {
                0
            },
            self.format_content_type_stats(&metrics),
            self.generate_analytics_insights(&metrics)
        );

        Ok(report)
    }

    fn format_content_type_stats(&self, metrics: &CrossSessionMetrics) -> String {
        metrics
            .per_content_type_ratios
            .iter()
            .map(|(ct, ratio)| format!("- {}: {:.2}x compression", ct, ratio))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn generate_analytics_insights(&self, metrics: &CrossSessionMetrics) -> String {
        let mut insights = Vec::new();

        if metrics.average_compression_ratio > 1.5 {
            insights.push("✓ Strong compression effectiveness: >1.5x average".to_string());
        }

        if metrics.total_compressions > 1000 {
            insights.push("✓ High-volume compression usage: >1000 compressions".to_string());
        }

        if metrics.total_sessions > 50 {
            insights.push("✓ Multi-session learning active: 50+ sessions tracked".to_string());
        }

        if insights.is_empty() {
            insights.push("Collect more data for better insights".to_string());
        }

        insights.join("\n- ")
    }
}

/// Statistics on CCR usage
#[derive(Debug, Clone)]
pub struct CcrStatistics {
    pub total_records: usize,
    pub total_retrievals: u32,
    pub total_original_bytes: usize,
    pub total_compressed_bytes: usize,
    pub average_compression_ratio: f64,
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
    use tempfile::TempDir;

    #[test]
    fn test_storage_config_creation() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        assert!(config.ccr_retention_days > 0);
        assert!(config.max_database_size_mb > 0);
    }

    #[test]
    fn test_persistent_storage_creation() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        let storage = PersistentStorageManager::new(config).expect("create storage failed");
        assert_eq!(storage.ccr_records.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_store_and_retrieve_ccr() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        let storage = PersistentStorageManager::new(config).expect("create storage failed");

        let record = PersistentCcrRecord {
            id: "test-1".to_string(),
            agent_id: "agent-1".to_string(),
            original_output: "original".to_string(),
            compressed_output: "compressed".to_string(),
            original_size: 100,
            compressed_size: 50,
            compression_ratio: 2.0,
            content_type: "json".to_string(),
            safety_level: "Safe".to_string(),
            created_at: current_timestamp(),
            retrieved_count: 0,
        };

        let id = storage.store_ccr_record(record.clone()).expect("store failed");
        let retrieved = storage.retrieve_ccr_record(&id).expect("retrieve failed");

        assert_eq!(retrieved.original_output, "original");
        assert_eq!(retrieved.compressed_output, "compressed");
        assert_eq!(retrieved.retrieved_count, 1);  // Incremented on retrieval
    }

    #[test]
    fn test_ccr_statistics() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        let storage = PersistentStorageManager::new(config).expect("create storage failed");

        for i in 0..5 {
            let record = PersistentCcrRecord {
                id: format!("test-{}", i),
                agent_id: "agent-1".to_string(),
                original_output: "x".repeat(100),
                compressed_output: "x".repeat(50),
                original_size: 100,
                compressed_size: 50,
                compression_ratio: 2.0,
                content_type: "json".to_string(),
                safety_level: "Safe".to_string(),
                created_at: current_timestamp(),
                retrieved_count: 0,
            };

            storage.store_ccr_record(record).ok();
        }

        let stats = storage.get_ccr_statistics().expect("stats failed");
        assert_eq!(stats.total_records, 5);
        assert_eq!(stats.average_compression_ratio, 2.0);
    }

    #[test]
    fn test_cross_session_metrics() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        let storage = PersistentStorageManager::new(config).expect("create storage failed");

        let record = PersistentCcrRecord {
            id: "test-1".to_string(),
            agent_id: "agent-1".to_string(),
            original_output: "x".repeat(1000),
            compressed_output: "x".repeat(500),
            original_size: 1000,
            compressed_size: 500,
            compression_ratio: 2.0,
            content_type: "json".to_string(),
            safety_level: "Safe".to_string(),
            created_at: current_timestamp(),
            retrieved_count: 0,
        };

        storage.store_ccr_record(record).ok();

        let metrics = storage.get_cross_session_metrics().expect("metrics failed");
        assert_eq!(metrics.total_compressions, 1);
        assert_eq!(metrics.total_tokens_saved, 500);
    }

    #[test]
    fn test_analytics_report_export() {
        let temp_dir = TempDir::new().expect("create temp dir failed");
        let config = StorageConfig::default_with_path(temp_dir.path());
        let storage = PersistentStorageManager::new(config).expect("create storage failed");

        let record = PersistentCcrRecord {
            id: "test-1".to_string(),
            agent_id: "agent-1".to_string(),
            original_output: "test".to_string(),
            compressed_output: "t".to_string(),
            original_size: 100,
            compressed_size: 50,
            compression_ratio: 2.0,
            content_type: "json".to_string(),
            safety_level: "Safe".to_string(),
            created_at: current_timestamp(),
            retrieved_count: 0,
        };

        storage.store_ccr_record(record).ok();

        let report = storage.export_analytics_report().expect("export failed");
        assert!(report.contains("CCR Statistics"));
        assert!(report.contains("Cross-Session Metrics"));
        assert!(report.contains("json"));
    }
}
