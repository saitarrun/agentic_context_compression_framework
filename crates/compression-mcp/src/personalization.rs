/// Phase 3: Personalization framework
/// Per-agent compression profiles for optimized compression strategies
///
/// Learns which compression strategies work best for each agent
/// based on task types, content patterns, and success metrics.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

/// Agent profile containing personalized compression preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub compression_strategies: StrategyPreferences,
    pub content_preferences: ContentPreferences,
    pub performance_metrics: PerformanceMetrics,
    pub last_updated: u64,
}

/// Preferred compression strategies for this agent
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StrategyPreferences {
    /// Preferred compression threshold (bytes)
    pub compression_threshold: usize,

    /// Per-content-type aggressiveness (0.0 - 1.0)
    /// 0.0 = no compression, 1.0 = aggressive
    pub json_aggressiveness: f64,
    pub code_aggressiveness: f64,
    pub text_aggressiveness: f64,

    /// Whether to use cloud fallback for Kompress-base
    pub use_cloud_inference: bool,

    /// Safety level preference (conservative, moderate, aggressive)
    pub safety_level: String,

    /// Prefer quick compression over aggressive reduction
    pub prefer_speed: bool,
}

/// Content patterns this agent works with
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContentPreferences {
    /// Typical content types this agent deals with
    pub primary_content_types: Vec<String>,

    /// Task categories (code_review, data_analysis, etc.)
    pub task_categories: Vec<String>,

    /// Tools most frequently used
    pub frequent_tools: Vec<(String, f64)>,

    /// Content size distribution preferences
    pub preferred_content_size: String,  // small, medium, large, varied
}

/// Performance metrics for this agent's compression
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceMetrics {
    /// Success rate with current compression strategy
    pub success_rate: f64,

    /// Average accuracy (signal preservation)
    pub average_accuracy: f64,

    /// Tokens saved using personalized strategy
    pub tokens_saved_total: u64,

    /// Number of tasks completed
    pub tasks_completed: u64,

    /// Error rate (compression failures)
    pub error_rate: f64,
}

/// Personalization manager for multi-agent profiles
pub struct PersonalizationManager {
    profiles: Arc<Mutex<HashMap<String, AgentProfile>>>,
    default_strategy: StrategyPreferences,
}

impl PersonalizationManager {
    /// Create a new personalization manager
    pub fn new() -> Self {
        Self {
            profiles: Arc::new(Mutex::new(HashMap::new())),
            default_strategy: StrategyPreferences::default(),
        }
    }

    /// Get or create profile for an agent
    pub fn get_or_create_profile(&self, agent_id: &str) -> Result<AgentProfile, String> {
        let mut profiles = self
            .profiles
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(profile) = profiles.get(agent_id) {
            Ok(profile.clone())
        } else {
            let profile = AgentProfile {
                agent_id: agent_id.to_string(),
                compression_strategies: self.default_strategy.clone(),
                content_preferences: ContentPreferences::default(),
                performance_metrics: PerformanceMetrics::default(),
                last_updated: current_timestamp(),
            };

            profiles.insert(agent_id.to_string(), profile.clone());
            Ok(profile)
        }
    }

    /// Update agent profile with new metrics
    pub fn update_profile_metrics(
        &self,
        agent_id: &str,
        task_success: bool,
        accuracy: f64,
        tokens_saved: u64,
        content_type: &str,
    ) -> Result<(), String> {
        let mut profiles = self
            .profiles
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if let Some(profile) = profiles.get_mut(agent_id) {
            let metrics = &mut profile.performance_metrics;

            // Update metrics
            metrics.tasks_completed += 1;
            metrics.tokens_saved_total += tokens_saved;

            // Update success rate (exponential moving average)
            let old_success = metrics.success_rate * (metrics.tasks_completed - 1) as f64;
            metrics.success_rate = (old_success + if task_success { 1.0 } else { 0.0 })
                / metrics.tasks_completed as f64;

            // Update accuracy (exponential moving average)
            let old_accuracy = metrics.average_accuracy * (metrics.tasks_completed - 1) as f64;
            metrics.average_accuracy = (old_accuracy + accuracy) / metrics.tasks_completed as f64;

            // Update error rate
            if !task_success {
                metrics.error_rate =
                    (metrics.error_rate * (metrics.tasks_completed - 1) as f64 + 1.0)
                        / metrics.tasks_completed as f64;
            }

            // Update content preferences
            if !profile.content_preferences.primary_content_types.contains(&content_type.to_string())
            {
                profile.content_preferences.primary_content_types.push(content_type.to_string());
            }

            profile.last_updated = current_timestamp();
        }

        Ok(())
    }

    /// Recommend compression strategy based on agent history
    pub fn recommend_strategy(&self, agent_id: &str) -> Result<StrategyPreferences, String> {
        let profile = self.get_or_create_profile(agent_id)?;

        let mut strategy = StrategyPreferences::default();

        // Adjust threshold based on success rate
        if profile.performance_metrics.success_rate > 0.90 {
            strategy.compression_threshold = 500;  // Lower threshold = more aggressive
        } else if profile.performance_metrics.success_rate < 0.70 {
            strategy.compression_threshold = 2000;  // Higher threshold = conservative
        }

        // Adjust aggressiveness based on accuracy
        let accuracy = profile.performance_metrics.average_accuracy;
        strategy.json_aggressiveness = accuracy;
        strategy.code_aggressiveness = accuracy * 0.95;  // Slightly more conservative
        strategy.text_aggressiveness = accuracy * 0.90;  // More conservative

        // Prefer speed if many tasks completed successfully
        if profile.performance_metrics.tasks_completed > 100 {
            strategy.prefer_speed = true;
        }

        // Use cloud if many errors (benefit from latest models)
        if profile.performance_metrics.error_rate > 0.05 {
            strategy.use_cloud_inference = true;
        }

        Ok(strategy)
    }

    /// Get all profiles
    pub fn list_profiles(&self) -> Result<Vec<AgentProfile>, String> {
        let profiles = self
            .profiles
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        Ok(profiles.values().cloned().collect())
    }

    /// Get top-performing agents
    pub fn get_top_agents(&self, limit: usize) -> Result<Vec<AgentProfile>, String> {
        let mut profiles = self.list_profiles()?;

        profiles.sort_by(|a, b| {
            b.performance_metrics.success_rate.partial_cmp(&a.performance_metrics.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(profiles.into_iter().take(limit).collect())
    }

    /// Get profiles needing intervention (low success rate)
    pub fn get_struggling_agents(&self, threshold: f64) -> Result<Vec<AgentProfile>, String> {
        let profiles = self.list_profiles()?;

        Ok(profiles
            .into_iter()
            .filter(|p| {
                p.performance_metrics.success_rate < threshold
                    && p.performance_metrics.tasks_completed > 10
            })
            .collect())
    }

    /// Export agent strategies as examples for learning
    pub fn export_best_strategies(&self) -> Result<String, String> {
        let top_agents = self.get_top_agents(10)?;

        let mut export = String::from("# Top Agent Compression Strategies\n\n");

        for agent in top_agents {
            export.push_str(&format!(
                "Agent {}: Success Rate {:.2}%, Avg Accuracy {:.2}%\n",
                agent.agent_id,
                agent.performance_metrics.success_rate * 100.0,
                agent.performance_metrics.average_accuracy * 100.0
            ));

            export.push_str(&format!(
                "  JSON Aggressiveness: {:.2}\n",
                agent.compression_strategies.json_aggressiveness
            ));
            export.push_str(&format!(
                "  Code Aggressiveness: {:.2}\n",
                agent.compression_strategies.code_aggressiveness
            ));
            export.push_str(&format!(
                "  Threshold: {} bytes\n",
                agent.compression_strategies.compression_threshold
            ));
            export.push_str("\n");
        }

        Ok(export)
    }
}

impl Default for PersonalizationManager {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_strategy_preferences_defaults() {
        let strategy = StrategyPreferences::default();
        assert_eq!(strategy.compression_threshold, 0);
        assert_eq!(strategy.json_aggressiveness, 0.0);
        assert_eq!(strategy.safety_level, "");
    }

    #[test]
    fn test_personalization_manager_creation() {
        let manager = PersonalizationManager::new();
        let profile = manager.get_or_create_profile("agent_1").expect("get profile failed");
        assert_eq!(profile.agent_id, "agent_1");
    }

    #[test]
    fn test_update_profile_metrics() {
        let manager = PersonalizationManager::new();
        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");

        manager
            .update_profile_metrics("agent_1", true, 0.95, 100, "json")
            .expect("update failed");

        let profile = manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");
        assert_eq!(profile.performance_metrics.tasks_completed, 1);
        assert_eq!(profile.performance_metrics.success_rate, 1.0);
        assert_eq!(profile.performance_metrics.tokens_saved_total, 100);
    }

    #[test]
    fn test_success_rate_calculation() {
        let manager = PersonalizationManager::new();
        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");

        // 3 successes, 1 failure
        manager.update_profile_metrics("agent_1", true, 0.95, 100, "json").ok();
        manager.update_profile_metrics("agent_1", true, 0.94, 100, "json").ok();
        manager.update_profile_metrics("agent_1", false, 0.80, 50, "json").ok();
        manager.update_profile_metrics("agent_1", true, 0.93, 100, "json").ok();

        let profile = manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");
        assert_eq!(profile.performance_metrics.tasks_completed, 4);
        assert!((profile.performance_metrics.success_rate - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_recommend_strategy_high_success() {
        let manager = PersonalizationManager::new();
        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");

        // Simulate high success rate
        for _ in 0..10 {
            manager.update_profile_metrics("agent_1", true, 0.98, 100, "json").ok();
        }

        let strategy = manager
            .recommend_strategy("agent_1")
            .expect("recommend failed");
        assert_eq!(strategy.compression_threshold, 500);  // More aggressive
    }

    #[test]
    fn test_recommend_strategy_low_success() {
        let manager = PersonalizationManager::new();
        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");

        // Simulate low success rate
        for _ in 0..10 {
            manager.update_profile_metrics("agent_1", false, 0.70, 50, "json").ok();
        }

        let strategy = manager
            .recommend_strategy("agent_1")
            .expect("recommend failed");
        assert_eq!(strategy.compression_threshold, 2000);  // More conservative
    }

    #[test]
    fn test_get_top_agents() {
        let manager = PersonalizationManager::new();

        // Create agents with different success rates
        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");
        manager
            .get_or_create_profile("agent_2")
            .expect("get profile failed");
        manager
            .get_or_create_profile("agent_3")
            .expect("get profile failed");

        // Give agent_1 high success
        for _ in 0..10 {
            manager.update_profile_metrics("agent_1", true, 0.99, 100, "json").ok();
        }

        // Give agent_2 medium success
        for i in 0..10 {
            manager
                .update_profile_metrics("agent_2", i % 2 == 0, 0.90, 100, "json")
                .ok();
        }

        // Give agent_3 low success
        for _ in 0..10 {
            manager.update_profile_metrics("agent_3", false, 0.60, 50, "json").ok();
        }

        let top = manager.get_top_agents(2).expect("get_top_agents failed");
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].agent_id, "agent_1");  // Highest success
    }

    #[test]
    fn test_struggling_agents() {
        let manager = PersonalizationManager::new();

        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");
        manager
            .get_or_create_profile("agent_2")
            .expect("get profile failed");

        // agent_1: low success
        for _ in 0..15 {
            manager.update_profile_metrics("agent_1", false, 0.60, 50, "json").ok();
        }

        // agent_2: high success
        for _ in 0..15 {
            manager.update_profile_metrics("agent_2", true, 0.95, 100, "json").ok();
        }

        let struggling = manager
            .get_struggling_agents(0.80)
            .expect("get_struggling_agents failed");
        assert_eq!(struggling.len(), 1);
        assert_eq!(struggling[0].agent_id, "agent_1");
    }

    #[test]
    fn test_export_best_strategies() {
        let manager = PersonalizationManager::new();

        manager
            .get_or_create_profile("agent_1")
            .expect("get profile failed");

        for _ in 0..15 {
            manager.update_profile_metrics("agent_1", true, 0.95, 100, "json").ok();
        }

        let export = manager.export_best_strategies().expect("export failed");
        assert!(export.contains("agent_1"));
        assert!(export.contains("Success Rate"));
        assert!(export.contains("Aggressiveness"));
    }
}
