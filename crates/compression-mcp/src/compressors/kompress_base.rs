use super::Compressor;
use mcp_types::MpcError;

/// KompressBase: Text/prose compression.
/// Uses a language model (local or cloud-based) to compress general text
/// while preserving error messages and diagnostic information.
///
/// DECISION POINT (Issue #4, HITL):
/// This implementation provides a stub. The team must decide:
///
/// Option A: Local Inference
/// - Download and use ONNX Runtime model locally
/// - Requires: onnx-runtime crate, model download (~50MB)
/// - Pros: Fast, no external service, works offline
/// - Cons: Slower startup, more dependencies, model updates needed
/// - Recommended for: Self-contained deployments, sensitive data
///
/// Option B: Cloud API
/// - Call Kompress-base API (external service)
/// - Requires: HTTP client, API key, network access
/// - Pros: Latest models, minimal dependencies, simple
/// - Cons: Latency, external dependency, requires authentication
/// - Recommended for: Scalable deployments, always-online agents
///
/// Option C: Hybrid
/// - Try local first, fallback to cloud
/// - Provides resilience and performance trade-offs
/// - Most complex but most flexible
///
/// This stub implements Option A (local inference) interface for now.
pub struct KompressBase {
    /// Model path (when implemented)
    _model_path: Option<String>,
}

impl KompressBase {
    /// Create a new KompressBase compressor.
    /// TODO: Load model from path or cloud when decision is finalized.
    pub fn new() -> Self {
        Self { _model_path: None }
    }

    /// Create with custom model path.
    pub fn with_model_path(path: String) -> Self {
        Self {
            _model_path: Some(path),
        }
    }

    /// Critical patterns to preserve in text.
    const PRESERVE_PATTERNS: &'static [&'static str] = &[
        "error",
        "exception",
        "fatal",
        "panic",
        "failed",
        "timeout",
        "refused",
        "denied",
        "not found",
        "invalid",
        "unauthorized",
    ];

    /// Check if text contains critical information.
    fn has_critical_info(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::PRESERVE_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Extract critical lines (currently: lines with error patterns).
    fn extract_critical_lines(text: &str) -> Vec<String> {
        text.lines()
            .filter(|line| Self::has_critical_info(line))
            .map(|s| s.to_string())
            .collect()
    }

    /// Simple heuristic compression: remove duplicate lines and excessive whitespace.
    /// This is a placeholder for when actual model inference is implemented.
    fn heuristic_compress(text: &str) -> String {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !seen.contains(trimmed) {
                seen.insert(trimmed.to_string());
                result.push(trimmed.to_string());
            }
        }

        result.join("\n")
    }
}

impl Default for KompressBase {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for KompressBase {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        // For now: preserve critical info and remove duplicates
        // This is a heuristic stand-in for ML-based compression

        if Self::has_critical_info(content) {
            // Keep original if it has critical info
            // (In production, ML model would intelligently compress while preserving semantics)
            let heuristic = Self::heuristic_compress(content);
            let ratio = content.len() as f64 / heuristic.len().max(1) as f64;
            Ok((heuristic, ratio))
        } else {
            // Simple deduplication for non-critical text
            let compressed = Self::heuristic_compress(content);
            let ratio = content.len() as f64 / compressed.len().max(1) as f64;
            Ok((compressed, ratio))
        }
    }

    fn name(&self) -> &str {
        "KompressBase"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kompress_base_creation() {
        let compressor = KompressBase::new();
        assert_eq!(compressor.name(), "KompressBase");
    }

    #[test]
    fn test_kompress_base_with_model_path() {
        let compressor = KompressBase::with_model_path("/path/to/model".to_string());
        assert_eq!(compressor.name(), "KompressBase");
    }

    #[test]
    fn test_kompress_base_removes_duplicates() {
        let compressor = KompressBase::new();
        let input = "line 1\nline 1\nline 2\nline 1\nline 3\n";
        let (output, ratio) = compressor.compress(input).expect("compress failed");
        assert!(!output.contains("line 1\nline 1"));
        assert!(ratio > 1.0);
    }

    #[test]
    fn test_kompress_base_preserves_error_content() {
        let compressor = KompressBase::new();
        let input = "Normal log line\nError: connection failed\nMore normal output\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("Error"));
        assert!(output.contains("failed"));
    }

    #[test]
    fn test_kompress_base_has_critical_info() {
        assert!(KompressBase::has_critical_info("Error: failed operation"));
        assert!(KompressBase::has_critical_info("Exception raised"));
        assert!(KompressBase::has_critical_info("FATAL: system down"));
        assert!(!KompressBase::has_critical_info("Normal output"));
    }

    #[test]
    fn test_kompress_base_extract_critical_lines() {
        let text = "start\nError: something\nmiddle\nFailed to connect\nend\n";
        let critical = KompressBase::extract_critical_lines(text);
        assert_eq!(critical.len(), 2);
        assert!(critical[0].contains("Error"));
        assert!(critical[1].contains("Failed"));
    }

    #[test]
    fn test_kompress_base_default() {
        let compressor = KompressBase::default();
        assert_eq!(compressor.name(), "KompressBase");
    }

    #[test]
    fn test_kompress_base_heuristic_compress() {
        let input = "line 1\n\n\nline 2\nline 1\nline 3";
        let output = KompressBase::heuristic_compress(input);
        assert!(!output.contains("\n\n"));
        // Duplicates are removed
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 3); // Only unique lines
    }
}
