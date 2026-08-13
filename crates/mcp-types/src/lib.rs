use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MpcError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Compression error: {0}")]
    CompressionError(String),
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("Search error: {0}")]
    SearchError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressRequest {
    pub tool_name: String,
    pub raw_output: String,
    #[serde(default)]
    pub task_goal: Option<String>,
    #[serde(default)]
    pub budget_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressResponse {
    pub output_id: String,
    pub compressed_output: String,
    pub compression_ratio: f64,
    pub tokens_saved: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveRequest {
    pub output_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveResponse {
    pub original_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub output_id: String,
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

fn default_max_results() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: usize,
    pub content: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub output_id: String,
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsRequest {
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub tokens_saved: u64,
    pub accuracy_delta: f64,
    pub workload_reduction: f64,
}

/// Budget levels for Budget-Aware Context Management (BACM)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetLevel {
    Light,
    Balanced,
    Aggressive,
    Extreme,
}

impl Default for BudgetLevel {
    fn default() -> Self {
        BudgetLevel::Balanced
    }
}

impl BudgetLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "light" => BudgetLevel::Light,
            "aggressive" => BudgetLevel::Aggressive,
            "extreme" => BudgetLevel::Extreme,
            _ => BudgetLevel::Balanced,
        }
    }
}

/// Content type for routing compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    Json,
    Code,
    Text,
    Unknown,
}

impl ContentType {
    pub fn detect(content: &str) -> Self {
        let trimmed = content.trim();

        // Try JSON
        if (trimmed.starts_with('{') || trimmed.starts_with('['))
            && serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return ContentType::Json;
        }

        // Try code (stack traces, diffs, function signatures)
        if trimmed.contains("at ") || trimmed.contains("File \"")
            || trimmed.contains("line ") || trimmed.contains("---")
            || trimmed.contains("fn ") || trimmed.contains("pub fn ")
            || trimmed.contains("def ") || trimmed.contains("class ") {
            return ContentType::Code;
        }

        // Default to text
        ContentType::Text
    }
}
