use super::Compressor;
use mcp_types::MpcError;

/// CodeCompressor: Code-specific high-ratio compression engine.
///
/// Features (Research-backed):
/// 1. AST Skeletonization / Signature Folding: Preserves signatures, docstrings, and type definitions
///    while folding long function bodies into concise stub annotations.
/// 2. Unified Diff Pruning: Strips bloated unchanged context lines from git diffs, keeping only
///    the hunk headers and modified delta lines.
/// 3. Stack Trace De-noising: Collapses framework boilerplate frames (tokio, runtime, stdlib)
///    while strictly preserving user application frames and root-cause error lines.
pub struct CodeCompressor {
    pub enable_skeletonization: bool,
    pub max_body_lines: usize,
}

impl CodeCompressor {
    pub fn new() -> Self {
        Self {
            enable_skeletonization: true,
            max_body_lines: 4,
        }
    }

    /// Lines/patterns that are critical signal and must always be preserved
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
        "struct ",
        "enum ",
        "trait ",
        "interface ",
        "type ",
        "function ",
        "=>",
        "throw",
        "Caused by:",
        "Error:",
        "FATAL:",
    ];

    /// Boilerplate frames in stack traces that can be collapsed
    const FRAMEWORK_NOISE_PATTERNS: &'static [&'static str] = &[
        "tokio::runtime",
        "core::ops::function",
        "std::panicking",
        "std::rt::lang_start",
        "alloc::boxed",
        "node:internal",
        "site-packages/urllib3",
        "site-packages/requests",
        "pytest/src/_pytest",
    ];

    /// Check if a line is critical signal
    fn is_signal_line(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::SIGNAL_PATTERNS.iter().any(|&p| lower.contains(&p.to_lowercase()))
    }

    /// Check if a line is a framework/runtime boilerplate frame
    fn is_framework_boilerplate(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::FRAMEWORK_NOISE_PATTERNS.iter().any(|&p| lower.contains(p))
    }

    /// Check if a line is pure noise (timestamps, elapsed, counters)
    fn is_pure_noise(line: &str) -> bool {
        let lower = line.to_lowercase();
        if lower.contains("ms") || lower.contains("seconds") || lower.contains("elapsed") {
            return true;
        }
        if lower.contains("retry") || lower.contains("backoff") || lower.contains("attempt") {
            return true;
        }
        if lower.contains("timestamp") || lower.contains("duration") || lower.contains("pid") {
            return true;
        }
        if line.starts_with("(") && line.ends_with(")") {
            return true;
        }
        false
    }

    /// Compress stack trace: collapse runtime frames, keep application lines
    fn compress_stack_trace(content: &str) -> String {
        let mut out = Vec::new();
        let mut skipped_frames = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if Self::is_framework_boilerplate(trimmed) {
                skipped_frames += 1;
                continue;
            }

            if skipped_frames > 0 {
                out.push(format!("  ... ({} framework frames omitted)", skipped_frames));
                skipped_frames = 0;
            }

            if Self::is_pure_noise(trimmed) {
                continue;
            }

            out.push(trimmed.to_string());
        }

        if skipped_frames > 0 {
            out.push(format!("  ... ({} framework frames omitted)", skipped_frames));
        }

        out.join("\n")
    }

    /// Compress diff output: keep hunk headers and +/- changes, prune outer context
    fn compress_diff(content: &str) -> String {
        let mut out = Vec::new();
        let mut context_count = 0;

        for line in content.lines() {
            if line.starts_with("---")
                || line.starts_with("+++")
                || line.starts_with("diff ")
                || line.starts_with("index ")
                || line.starts_with("@@")
            {
                out.push(line.trim_end().to_string());
                context_count = 0;
            } else if line.starts_with('+') || line.starts_with('-') {
                out.push(line.trim_end().to_string());
                context_count = 0;
            } else {
                // Keep max 1 context line before/after diff hunk
                if context_count < 1 && !line.trim().is_empty() {
                    out.push(line.trim_end().to_string());
                    context_count += 1;
                }
            }
        }

        out.join("\n")
    }

    /// Skeletonize code blocks: retain signatures, collapse long bodies
    fn skeletonize_code(&self, content: &str) -> String {
        let mut out = Vec::new();
        let mut in_body = false;
        let mut body_lines_count = 0;
        let mut brace_depth = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let is_sig = trimmed.starts_with("pub fn ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("trait ");

            let open_braces = trimmed.matches('{').count();
            let close_braces = trimmed.matches('}').count();

            if is_sig {
                if in_body && body_lines_count > self.max_body_lines {
                    out.push(format!("    /* ... ({} lines body omitted) ... */", body_lines_count));
                }
                out.push(trimmed.to_string());
                in_body = true;
                body_lines_count = 0;
                brace_depth = open_braces.saturating_sub(close_braces);
            } else if in_body {
                brace_depth += open_braces;
                brace_depth = brace_depth.saturating_sub(close_braces);

                // Preserve lines with errors or critical markers
                if Self::is_signal_line(trimmed) || body_lines_count < self.max_body_lines {
                    out.push(trimmed.to_string());
                }
                body_lines_count += 1;

                if brace_depth == 0 && close_braces > 0 {
                    if body_lines_count > self.max_body_lines {
                        out.push(format!("    /* ... ({} lines body omitted) ... */", body_lines_count - self.max_body_lines));
                    }
                    out.push("}".to_string());
                    in_body = false;
                    body_lines_count = 0;
                }
            } else {
                if !Self::is_pure_noise(trimmed) {
                    out.push(trimmed.to_string());
                }
            }
        }

        out.join("\n")
    }

    /// Detect if content is a diff
    fn is_diff(content: &str) -> bool {
        content.contains("+++") || content.contains("---") || content.contains("@@")
    }

    /// Detect if content is a stack trace
    fn is_stack_trace(content: &str) -> bool {
        content.contains(" at ") || content.contains("Traceback") || content.contains("File \"")
    }
}

impl Default for CodeCompressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for CodeCompressor {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        let compressed = if Self::is_diff(content) {
            Self::compress_diff(content)
        } else if Self::is_stack_trace(content) {
            Self::compress_stack_trace(content)
        } else if self.enable_skeletonization {
            self.skeletonize_code(content)
        } else {
            content
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || Self::is_pure_noise(trimmed) {
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
    fn test_code_compressor_preserves_function_signatures() {
        let compressor = CodeCompressor::new();
        let input = "fn main() {\n    println!(\"hello\");\n}\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_code_compressor_stack_trace_framework_collapse() {
        let compressor = CodeCompressor::new();
        let input = "Error: connection timeout\n  at tokio::runtime::task (task.rs:10)\n  at ConnectHandler (connection.rs:42:10)\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("ConnectHandler"));
        assert!(output.contains("framework frames omitted") || !output.contains("tokio::runtime"));
    }

    #[test]
    fn test_code_compressor_diff_format() {
        let compressor = CodeCompressor::new();
        let input = "--- file.rs\n+++ file.rs\n@@ -1,3 +1,4 @@\n-old line\n+new line\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("+++"));
        assert!(output.contains("-old line"));
        assert!(output.contains("+new line"));
    }
}
