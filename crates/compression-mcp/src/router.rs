use crate::compressors::{CodeCompressor, Compressor, KompressBase, SmartCrusher};
use mcp_types::{ContentType, MpcError};
use std::collections::HashMap;
use std::sync::Arc;

/// ContentRouter: Routes compression requests to appropriate compressors.
/// Detects content type and selects the right compression algorithm.
pub struct ContentRouter {
    compressors: HashMap<ContentType, Arc<dyn Compressor>>,
}

impl ContentRouter {
    /// Create a new ContentRouter with default compressors.
    pub fn new() -> Self {
        let mut compressors: HashMap<ContentType, Arc<dyn Compressor>> = HashMap::new();

        compressors.insert(ContentType::Json, Arc::new(SmartCrusher) as Arc<dyn Compressor>);
        compressors.insert(ContentType::Code, Arc::new(CodeCompressor) as Arc<dyn Compressor>);
        compressors.insert(ContentType::Text, Arc::new(KompressBase::new()) as Arc<dyn Compressor>);

        Self { compressors }
    }

    /// Register a custom compressor for a content type.
    pub fn register(&mut self, content_type: ContentType, compressor: Arc<dyn Compressor>) {
        self.compressors.insert(content_type, compressor);
    }

    /// Route and compress content based on detected type.
    pub fn compress(&self, content: &str) -> Result<(String, f64, ContentType), MpcError> {
        let content_type = ContentType::detect(content);
        let compressor = self
            .compressors
            .get(&content_type)
            .ok_or_else(|| MpcError::UnknownTool(format!("No compressor for {:?}", content_type)))?;

        let (compressed, ratio) = compressor.compress(content)?;
        Ok((compressed, ratio, content_type))
    }

    /// Get all registered compressors.
    pub fn compressors(&self) -> Vec<&str> {
        self.compressors.values().map(|c| c.name()).collect()
    }
}

impl Default for ContentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_router_creation() {
        let router = ContentRouter::new();
        let compressor_names: Vec<&str> = router.compressors();
        assert_eq!(compressor_names.len(), 3);
        assert!(compressor_names.iter().any(|&name| name == "SmartCrusher"));
        assert!(compressor_names.iter().any(|&name| name == "CodeCompressor"));
        assert!(compressor_names.iter().any(|&name| name == "KompressBase"));
    }

    #[test]
    fn test_router_json_detection_and_routing() {
        let router = ContentRouter::new();
        let json_input = r#"{"status": "ok"}"#;
        let (output, _ratio, content_type) = router.compress(json_input).expect("compress failed");
        assert_eq!(content_type, ContentType::Json);
        assert_eq!(output, r#"{"status":"ok"}"#);
    }

    #[test]
    fn test_router_code_detection_and_routing() {
        let router = ContentRouter::new();
        let code_input = "at function () (file.rs:42:10)";
        let (output, _ratio, content_type) = router.compress(code_input).expect("compress failed");
        assert_eq!(content_type, ContentType::Code);
        assert_eq!(output, code_input);
    }

    #[test]
    fn test_router_text_detection_and_routing() {
        let router = ContentRouter::new();
        let text_input = "This is plain text output";
        let (output, _ratio, content_type) = router.compress(text_input).expect("compress failed");
        assert_eq!(content_type, ContentType::Text);
        assert_eq!(output, text_input);
    }

    #[test]
    fn test_router_register_custom_compressor() {
        let mut router = ContentRouter::new();
        struct DummyCompressor;
        impl Compressor for DummyCompressor {
            fn compress(&self, _: &str) -> Result<(String, f64), MpcError> {
                Ok(("compressed".to_string(), 0.5))
            }
            fn name(&self) -> &str {
                "DummyCompressor"
            }
        }
        router.register(ContentType::Unknown, Arc::new(DummyCompressor));
        assert!(router.compressors().iter().any(|&name| name == "DummyCompressor"));
    }
}
