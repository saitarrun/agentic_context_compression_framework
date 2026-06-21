/// Signal maps define tool-specific rules for compression.
/// Each tool type has different signal/noise patterns.
///
/// Signal = important data (results, errors, file info)
/// Noise = metadata (timestamps, retry counts, duration)

use std::collections::HashSet;

/// Signal map for shell command outputs.
pub struct ShellSignalMap;

impl ShellSignalMap {
    /// Critical shell output patterns to preserve.
    const SIGNAL_PATTERNS: &'static [&'static str] = &[
        "error",
        "failed",
        "cannot",
        "no such file",
        "permission denied",
        "command not found",
        "exit code",
        "exit status",
        "stderr",
        "stdout",
        "result",
        "output",
        "success",
    ];

    /// Noise patterns to remove from shell output.
    const NOISE_PATTERNS: &'static [&'static str] = &[
        "ms",
        "seconds",
        "elapsed",
        "duration",
        "took",
        "retry",
        "attempt",
        "backoff",
        "timeout after",
        "pid:",
        "process id",
    ];

    /// Check if a line contains shell signal.
    pub fn is_signal(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::SIGNAL_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Check if a line is noise.
    pub fn is_noise(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::NOISE_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Compress shell output preserving signal.
    pub fn compress(content: &str) -> String {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (Self::is_signal(trimmed) || !Self::is_noise(trimmed))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Signal map for file operation outputs.
pub struct FileOpsSignalMap;

impl FileOpsSignalMap {
    /// Critical file operation signal.
    const SIGNAL_PATTERNS: &'static [&'static str] = &[
        "path",
        "file",
        "directory",
        "permission",
        "mode",
        "owner",
        "group",
        "size",
        "bytes",
        "error",
        "failed",
        "cannot",
        "denied",
        "not found",
        "exists",
        "modified",
    ];

    /// Noise in file ops.
    const NOISE_PATTERNS: &'static [&'static str] = &[
        "inode",
        "accessed",
        "changed",
        "timestamp",
        "uid",
        "gid",
        "dev",
        "rdev",
        "blocks",
    ];

    /// Check if a line contains file op signal.
    pub fn is_signal(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::SIGNAL_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Check if a line is noise.
    pub fn is_noise(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::NOISE_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Compress file ops preserving important info.
    pub fn compress(content: &str) -> String {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (Self::is_signal(trimmed) || !Self::is_noise(trimmed))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Signal map for HTTP fetch outputs.
pub struct FetchSignalMap;

impl FetchSignalMap {
    /// Critical fetch/API response signal.
    const SIGNAL_PATTERNS: &'static [&'static str] = &[
        "status",
        "statuscode",
        "code",
        "body",
        "response",
        "data",
        "error",
        "message",
        "content-type",
        "content-length",
        "result",
        "success",
        "failed",
        "timeout",
        "connection",
    ];

    /// Noise in fetch responses.
    const NOISE_PATTERNS: &'static [&'static str] = &[
        "x-",
        "date:",
        "server:",
        "set-cookie",
        "transfer-encoding",
        "cache-control",
        "etag",
        "expires",
        "last-modified",
        "connection:",
        "keep-alive",
        "ms",
        "ms elapsed",
        "retry",
        "attempt",
    ];

    /// Check if a line contains fetch signal.
    pub fn is_signal(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::SIGNAL_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Check if a line is noise.
    pub fn is_noise(line: &str) -> bool {
        let lower = line.to_lowercase();
        Self::NOISE_PATTERNS
            .iter()
            .any(|&p| lower.contains(p))
    }

    /// Compress fetch response preserving important info.
    pub fn compress(content: &str) -> String {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && (Self::is_signal(trimmed) || !Self::is_noise(trimmed))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_signal_detection() {
        assert!(ShellSignalMap::is_signal("error: command failed"));
        assert!(ShellSignalMap::is_signal("exit code 1"));
        assert!(!ShellSignalMap::is_signal("retry attempt 3"));
    }

    #[test]
    fn test_shell_compress() {
        let input = "line 1\nerror: connection failed\nretry attempt 3\nstderr: critical\n";
        let output = ShellSignalMap::compress(input);
        assert!(output.contains("error"));
        assert!(output.contains("stderr"));
        assert!(!output.contains("retry"));
    }

    #[test]
    fn test_fileops_signal_detection() {
        assert!(FileOpsSignalMap::is_signal("path: /tmp/file"));
        assert!(FileOpsSignalMap::is_signal("permission denied"));
        assert!(!FileOpsSignalMap::is_signal("inode: 12345"));
    }

    #[test]
    fn test_fileops_compress() {
        let input = "file: /tmp/test\ninode: 999999\nsize: 1024\ntimestamp: 1234567890\n";
        let output = FileOpsSignalMap::compress(input);
        assert!(output.contains("file"));
        assert!(output.contains("size"));
        assert!(!output.contains("inode"));
        assert!(!output.contains("timestamp"));
    }

    #[test]
    fn test_fetch_signal_detection() {
        assert!(FetchSignalMap::is_signal("status: 200"));
        assert!(FetchSignalMap::is_signal("content-type: application/json"));
        assert!(!FetchSignalMap::is_signal("x-custom-header: value"));
    }

    #[test]
    fn test_fetch_compress() {
        let input = "status: 200\nx-request-id: abc123\nbody: {\"data\": \"result\"}\nx-rate-limit: 1000\n";
        let output = FetchSignalMap::compress(input);
        assert!(output.contains("status"));
        assert!(output.contains("body"));
        assert!(!output.contains("x-request-id"));
        assert!(!output.contains("x-rate-limit"));
    }
}
