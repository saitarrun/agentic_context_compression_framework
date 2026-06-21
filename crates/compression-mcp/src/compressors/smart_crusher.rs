use super::Compressor;
use mcp_types::MpcError;
use serde_json::{json, Value};

/// SmartCrusher: JSON-specific compression.
/// Preserves signal fields (keys, values, structure) while removing noise:
/// - Removes whitespace (compact formatting)
/// - Removes null values (unless critical)
/// - Removes empty arrays and objects
/// - Preserves error messages and result data
pub struct SmartCrusher;

impl SmartCrusher {
    /// Signal fields that should be preserved even if they seem like noise.
    const SIGNAL_FIELDS: &'static [&'static str] = &[
        "error",
        "message",
        "status",
        "code",
        "result",
        "data",
        "stderr",
        "stdout",
        "exit_code",
    ];

    /// Compress JSON by removing unnecessary elements while preserving signal.
    fn compress_value(value: &Value) -> Option<Value> {
        match value {
            Value::Null => None,
            Value::Object(obj) => {
                let mut compressed = serde_json::Map::new();
                for (key, val) in obj {
                    // Check if this is a signal field or has signal content
                    if Self::is_signal_field(key) || !Self::is_noise_value(val) {
                        if let Some(compressed_val) = Self::compress_value(val) {
                            compressed.insert(key.clone(), compressed_val);
                        } else if Self::is_signal_field(key) {
                            // Preserve signal fields even if value is null
                            compressed.insert(key.clone(), Value::Null);
                        }
                    }
                }
                if compressed.is_empty() {
                    None
                } else {
                    Some(Value::Object(compressed))
                }
            }
            Value::Array(arr) => {
                let compressed: Vec<Value> = arr
                    .iter()
                    .filter_map(Self::compress_value)
                    .collect();
                if compressed.is_empty() {
                    None
                } else {
                    Some(Value::Array(compressed))
                }
            }
            _ => Some(value.clone()),
        }
    }

    /// Check if a field name is likely to contain signal (important data).
    fn is_signal_field(field: &str) -> bool {
        let lower = field.to_lowercase();
        Self::SIGNAL_FIELDS.iter().any(|&f| lower.contains(f))
            || lower.contains("id")
            || lower.contains("name")
    }

    /// Check if a value is likely noise.
    fn is_noise_value(val: &Value) -> bool {
        match val {
            Value::String(s) => {
                // Common noise patterns: timestamps, UUIDs, retry info
                s.contains("ms") || s.contains("seconds") || s.contains("2024") || s.contains("2025")
                    || s.contains("retry")
            }
            Value::Number(n) => {
                // Very large numbers (likely timestamps or counters)
                if let Some(u) = n.as_u64() {
                    u > 1_000_000_000 // Likely Unix timestamp
                } else {
                    false
                }
            }
            Value::Null => true,
            Value::Object(obj) => obj.is_empty(),
            Value::Array(arr) => arr.is_empty(),
            _ => false,
        }
    }
}

impl Compressor for SmartCrusher {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        // Try to parse as JSON
        let value: Value = serde_json::from_str(content)
            .map_err(|e| MpcError::CompressionError(format!("Invalid JSON: {}", e)))?;

        // Compress the JSON
        let compressed_value = Self::compress_value(&value)
            .ok_or_else(|| MpcError::CompressionError("JSON compressed to empty".to_string()))?;

        // Convert to compact JSON (no whitespace)
        let compressed = compressed_value.to_string();

        let original_len = content.len() as f64;
        let compressed_len = compressed.len() as f64;
        let ratio = original_len / compressed_len;

        Ok((compressed, ratio))
    }

    fn name(&self) -> &str {
        "SmartCrusher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_crusher_removes_whitespace() {
        let crusher = SmartCrusher;
        let input = r#"{
            "status": "ok",
            "data": [1, 2, 3]
        }"#;
        let (output, _ratio) = crusher.compress(input).expect("compress failed");
        assert!(output.len() < input.len());
        assert!(output.contains("\"status\":\"ok\""));
    }

    #[test]
    fn test_smart_crusher_removes_null_values() {
        let crusher = SmartCrusher;
        let input = r#"{"status":"ok","error":null,"data":"result"}"#;
        let (output, _ratio) = crusher.compress(input).expect("compress failed");
        // Signal fields like "error" are preserved even if null
        assert!(output.contains("\"error\":null"));
    }

    #[test]
    fn test_smart_crusher_preserves_signal_fields() {
        let crusher = SmartCrusher;
        let input = r#"{"status":"error","message":"connection timeout","retry_count":5}"#;
        let (output, _ratio) = crusher.compress(input).expect("compress failed");
        assert!(output.contains("status"));
        assert!(output.contains("message"));
    }

    #[test]
    fn test_smart_crusher_removes_empty_objects() {
        let crusher = SmartCrusher;
        let input = r#"{"status":"ok","metadata":{},"data":"result"}"#;
        let (output, _ratio) = crusher.compress(input).expect("compress failed");
        assert!(!output.contains("metadata"));
    }

    #[test]
    fn test_smart_crusher_calculates_ratio() {
        let crusher = SmartCrusher;
        let input = r#"{
            "status": "ok",
            "result": "success"
        }"#;
        let (_output, ratio) = crusher.compress(input).expect("compress failed");
        assert!(ratio > 1.0, "Compression ratio should be > 1");
    }

    #[test]
    fn test_smart_crusher_name() {
        let crusher = SmartCrusher;
        assert_eq!(crusher.name(), "SmartCrusher");
    }

    #[test]
    fn test_smart_crusher_invalid_json() {
        let crusher = SmartCrusher;
        let result = crusher.compress("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_smart_crusher_preserves_error_message() {
        let crusher = SmartCrusher;
        let input = r#"{"status":"error","message":"API key expired","timestamp":1720000000000,"retry_count":3}"#;
        let (output, _ratio) = crusher.compress(input).expect("compress failed");
        assert!(output.contains("message"));
        assert!(output.contains("API key expired"));
        // Noise fields should be removed
        assert!(!output.contains("retry_count"));
    }
}
