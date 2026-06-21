use super::Compressor;
use mcp_types::MpcError;

/// CodeCompressor: Code-specific compression.
/// Preserves signal elements (function signatures, line numbers, error messages)
/// while removing noise (timestamps, retry counts, formatting).
///
/// Handles:
/// - Stack traces (preserves function names and line numbers)
/// - Diffs (preserves changed lines, removes context)
/// - Error messages (preserves error type and location)
/// - Source code (preserves structure and function sigs)
pub struct CodeCompressor;

impl CodeCompressor {
    /// Lines/patterns that are critical signal and must be preserved
    const SIGNAL_PATTERNS: &'static [&'static str] = &[
        "at ",
        "line ",
        "error",
        "panic",
        "exception",
        "traceback",
        "File \"",
        "def ",
        "fn ",
        "pub ",
        "class ",
        "function ",
        "=>",
        "throw",
        "Caused by:",
        "Error:",
        "FATAL:",
    ];

    /// Check if a line contains signal information.
    fn is_signal_line(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::SIGNAL_PATTERNS.iter().any(|&p| lower.contains(&p.to_lowercase()))
    }

    /// Remove noise from a code line while preserving signal.
    fn compress_line(line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            return None;
        }

        // Preserve signal lines as-is
        if Self::is_signal_line(trimmed) {
            return Some(trimmed.to_string());
        }

        // Skip lines that are pure timestamps or metadata
        if Self::is_pure_noise(trimmed) {
            return None;
        }

        // Keep other lines (source code, comments, etc.)
        Some(trimmed.to_string())
    }

    /// Check if a line is pure noise (no signal).
    fn is_pure_noise(line: &str) -> bool {
        let lower = line.to_lowercase();

        // Timestamp patterns
        if lower.contains("ms") || lower.contains("seconds") || lower.contains("elapsed") {
            return true;
        }

        // Retry/backoff patterns
        if lower.contains("retry") || lower.contains("backoff") || lower.contains("attempt") {
            return true;
        }

        // Metadata patterns
        if lower.contains("timestamp") || lower.contains("duration") || lower.contains("pid") {
            return true;
        }

        // Request/response metadata
        if line.starts_with("(") && line.ends_with(")") {
            return true;
        }

        false
    }

    /// Compress stack trace format (multiple lines).
    fn compress_stack_trace(content: &str) -> String {
        content
            .lines()
            .filter_map(Self::compress_line)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Compress diff output.
    fn compress_diff(content: &str) -> String {
        content
            .lines()
            .filter(|line| {
                // Keep header lines and changed lines
                line.starts_with("@@")
                    || line.starts_with("+")
                    || line.starts_with("-")
                    || line.starts_with("---")
                    || line.starts_with("+++")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Detect if content is a diff.
    fn is_diff(content: &str) -> bool {
        content.contains("+++") || content.contains("---") || content.contains("@@")
    }

    /// Detect if content is a stack trace.
    fn is_stack_trace(content: &str) -> bool {
        content.contains(" at ") || content.contains("Traceback") || content.contains("File \"")
    }
}

impl Compressor for CodeCompressor {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        let compressed = if Self::is_diff(content) {
            Self::compress_diff(content)
        } else if Self::is_stack_trace(content) {
            Self::compress_stack_trace(content)
        } else {
            // Default: remove empty lines and excessive whitespace
            content
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let original_len = content.len() as f64;
        let compressed_len = compressed.len() as f64;
        let ratio = if compressed_len > 0.0 {
            original_len / compressed_len
        } else {
            1.0
        };

        Ok((compressed, ratio))
    }

    fn name(&self) -> &str {
        "CodeCompressor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_compressor_removes_empty_lines() {
        let compressor = CodeCompressor;
        let input = "line 1\n\n\nline 2\n\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(!output.contains("\n\n"));
        assert!(output.contains("line 1"));
        assert!(output.contains("line 2"));
    }

    #[test]
    fn test_code_compressor_preserves_function_signatures() {
        let compressor = CodeCompressor;
        let input = "fn main() {\n    println!(\"hello\");\n}\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_code_compressor_stack_trace_preservation() {
        let compressor = CodeCompressor;
        let input = "Error: connection timeout\n  at ConnectHandler (connection.rs:42:10)\n  at retry (handler.rs:100:5)\nElapsed: 5000ms\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("at ConnectHandler"));
        assert!(output.contains("connection.rs:42:10"));
        assert!(!output.contains("Elapsed"));
    }

    #[test]
    fn test_code_compressor_diff_format() {
        let compressor = CodeCompressor;
        let input = "--- file.rs\n+++ file.rs\n@@ -1,3 +1,4 @@\n-old line\n+new line\ncommon line\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("+++"));
        assert!(output.contains("-old line"));
        assert!(output.contains("+new line"));
        assert!(!output.contains("common line"));
    }

    #[test]
    fn test_code_compressor_name() {
        let compressor = CodeCompressor;
        assert_eq!(compressor.name(), "CodeCompressor");
    }

    #[test]
    fn test_code_compressor_calculates_ratio() {
        let compressor = CodeCompressor;
        let input = "line 1\n\n\n\nline 2\n\n\n";
        let (_, ratio) = compressor.compress(input).expect("compress failed");
        assert!(ratio > 1.0);
    }

    #[test]
    fn test_is_signal_line() {
        assert!(CodeCompressor::is_signal_line("  at function (file.rs:42:10)"));
        assert!(CodeCompressor::is_signal_line("Error: something failed"));
        assert!(CodeCompressor::is_signal_line("fn my_function() {"));
        assert!(!CodeCompressor::is_signal_line("regular comment"));
    }

    #[test]
    fn test_is_pure_noise() {
        assert!(CodeCompressor::is_pure_noise("Elapsed: 5000ms"));
        assert!(CodeCompressor::is_pure_noise("Retry attempt 3"));
        assert!(CodeCompressor::is_pure_noise("(request metadata)"));
        assert!(!CodeCompressor::is_pure_noise("Error: failed"));
    }
}
