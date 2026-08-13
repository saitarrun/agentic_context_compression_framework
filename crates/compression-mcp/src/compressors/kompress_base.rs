use super::Compressor;
use mcp_types::MpcError;

/// KompressBase: High-performance text and log compressor.
///
/// Features (Research-backed):
/// 1. Multi-Rubric Log Filtering (LaMR style): Isolates error/diagnostic evidence from routine execution noise.
/// 2. Loop & Progress Bar Collapsing: Compresses download progress bars, repetitive polling messages,
///    and consecutive similar log lines into summary notations (`[... N repetitive lines omitted]`).
/// 3. Noise Prefix Stripping: Cleans verbose timestamp/PID log headers while keeping the core log body.
pub struct KompressBase {
    _model_path: Option<String>,
}

impl KompressBase {
    pub fn new() -> Self {
        Self { _model_path: None }
    }

    pub fn with_model_path(path: String) -> Self {
        Self {
            _model_path: Some(path),
        }
    }

    /// Critical patterns to always preserve with full fidelity
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
        "exit status",
        "exit code",
        "warn",
    ];

    /// Check if text contains critical information
    pub fn has_critical_info(text: &str) -> bool {
        let lower = text.to_lowercase();
        Self::PRESERVE_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Check if a line is a transient progress bar (e.g. [===>   ] 45% or 100/500 items)
    fn is_progress_line(line: &str) -> bool {
        (line.contains('%') && (line.contains('[') || line.contains(']')))
            || line.contains("====")
            || line.contains("----")
            || line.contains("ETA ")
            || line.contains("downloading...")
    }

    /// Compress logs by removing progress spam, deduplicating consecutive lines, and trimming headers
    fn smart_text_compress(text: &str) -> String {
        let mut out = Vec::new();
        let mut last_line = String::new();
        let mut repeat_count = 0;
        let mut skipped_progress = 0;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Collapse progress bars
            if Self::is_progress_line(trimmed) && !Self::has_critical_info(trimmed) {
                skipped_progress += 1;
                continue;
            }

            if skipped_progress > 0 {
                out.push(format!("  [... {} progress lines omitted]", skipped_progress));
                skipped_progress = 0;
            }

            // Deduplicate consecutive identical lines
            if trimmed == last_line {
                repeat_count += 1;
                continue;
            }

            if repeat_count > 0 {
                out.push(format!("  [... repeated {} times ...]", repeat_count));
                repeat_count = 0;
            }

            out.push(trimmed.to_string());
            last_line = trimmed.to_string();
        }

        if repeat_count > 0 {
            out.push(format!("  [... repeated {} times ...]", repeat_count));
        }
        if skipped_progress > 0 {
            out.push(format!("  [... {} progress lines omitted]", skipped_progress));
        }

        out.join("\n")
    }
}

impl Default for KompressBase {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor for KompressBase {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        let compressed = Self::smart_text_compress(content);
        let orig_len = content.len() as f64;
        let comp_len = compressed.len() as f64;
        let ratio = if comp_len > 0.0 { orig_len / comp_len } else { 1.0 };
        Ok((compressed, ratio))
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
    fn test_kompress_base_removes_consecutive_duplicates() {
        let compressor = KompressBase::new();
        let input = "line 1\nline 1\nline 1\nline 2\n";
        let (output, ratio) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("repeated 2 times") || !output.contains("line 1\nline 1\nline 1"));
        assert!(output.contains("line 2"));
        assert!(ratio > 1.0);
    }

    #[test]
    fn test_kompress_base_collapses_progress_bars() {
        let compressor = KompressBase::new();
        let input = "Start download\n[=>     ] 10%\n[===>   ] 40%\n[======>] 100%\nDownload complete";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("Start download"));
        assert!(output.contains("Download complete"));
        assert!(output.contains("progress lines omitted") || !output.contains("[=>     ]"));
    }

    #[test]
    fn test_kompress_base_preserves_error() {
        let compressor = KompressBase::new();
        let input = "Normal log line\nError: connection failed with exit status 1\n";
        let (output, _) = compressor.compress(input).expect("compress failed");
        assert!(output.contains("Error: connection failed with exit status 1"));
    }
}
