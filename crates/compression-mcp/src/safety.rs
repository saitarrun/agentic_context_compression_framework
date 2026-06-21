/// Safety invariants for compression.
/// These rules ensure that critical information is never compressed or leaked.
///
/// Critical Invariants (from PRD):
/// 1. Byte-faithful passthrough: Unmodified content arrives at LLM byte-equal
/// 2. Append-only compression: Only live zone modified, cache-hot zone untouched
/// 3. Deterministic compression: Same input → same output
/// 4. Signal preservation: Error messages, function sigs, auth metadata never compressed
/// 5. CCR coverage: Every compressed output has retrievable original

// Authentication-related keywords (never compress)
const AUTH_KEYWORDS: &[&str] = &[
    "authorization",
    "auth",
    "bearer",
    "token",
    "api_key",
    "secret",
    "password",
    "credential",
    "api-key",
    "x-api-key",
    "aws-secret",
    "access-key",
];

// Error/critical keywords (always preserve)
const CRITICAL_KEYWORDS: &[&str] = &[
    "error:",
    "exception:",
    "fatal:",
    "panic:",
    "failed:",
    "failure:",
    "stderr:",
    "warning:",
];

// Tool definition patterns (never compress)
const TOOL_DEF_PATTERNS: &[&str] = &[
    "\"name\":",
    "\"description\":",
    "\"inputSchema\":",
    "\"type\":",
    "\"properties\":",
    "\"required\":",
    "\"enum\":",
];

// Function signature patterns (preserve)
const FUNCTION_SIG_PATTERNS: &[&str] = &[
    "fn ",
    "pub fn",
    "async fn",
    "def ",
    "function ",
    "class ",
    "struct ",
    "impl ",
    "trait ",
];

/// Safety check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Safe to compress - no critical content detected
    Safe,
    /// Risky - contains some sensitive data, compress with caution
    Risky,
    /// Unsafe - contains critical auth or tool data, do not compress
    Unsafe,
}

/// Check if content contains authentication or security-sensitive data.
pub fn has_auth_data(content: &str) -> bool {
    let lower = content.to_lowercase();
    AUTH_KEYWORDS.iter().any(|keyword| lower.contains(keyword))
}

/// Check if content contains critical error information.
pub fn has_critical_errors(content: &str) -> bool {
    let lower = content.to_lowercase();
    CRITICAL_KEYWORDS
        .iter()
        .any(|keyword| lower.contains(keyword))
}

/// Check if content contains tool definitions (function sigs, MCP tool specs).
pub fn has_tool_definitions(content: &str) -> bool {
    TOOL_DEF_PATTERNS
        .iter()
        .any(|pattern| content.contains(pattern))
}

/// Check if content contains function signatures.
pub fn has_function_signatures(content: &str) -> bool {
    FUNCTION_SIG_PATTERNS
        .iter()
        .any(|pattern| content.contains(pattern))
}

/// Comprehensive safety assessment.
pub fn assess_safety(content: &str) -> SafetyLevel {
    // Unsafe conditions (absolute no-compress)
    if has_auth_data(content) || has_tool_definitions(content) {
        return SafetyLevel::Unsafe;
    }

    // Risky conditions (allow compress, but check carefully)
    if has_critical_errors(content) || has_function_signatures(content) {
        return SafetyLevel::Risky;
    }

    SafetyLevel::Safe
}

/// Extract critical information that must be preserved.
pub fn extract_critical_lines(content: &str) -> Vec<String> {
    content
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            has_auth_data(line)
                || CRITICAL_KEYWORDS
                    .iter()
                    .any(|keyword| lower.contains(keyword))
                || TOOL_DEF_PATTERNS.iter().any(|pattern| line.contains(pattern))
        })
        .map(|s| s.to_string())
        .collect()
}

/// Verify that content doesn't contain leaked secrets (post-compression validation).
pub fn validate_no_secrets(content: &str) -> Result<(), String> {
    let problematic = extract_auth_patterns(content);
    if !problematic.is_empty() {
        Err(format!(
            "Found {} suspicious patterns (possible leaked secrets): {:?}",
            problematic.len(),
            problematic.get(0)
        ))
    } else {
        Ok(())
    }
}

/// Extract patterns that look like secrets.
fn extract_auth_patterns(content: &str) -> Vec<String> {
    let mut patterns = Vec::new();

    for line in content.lines() {
        // Bearer token pattern
        if line.contains("bearer ") && line.len() > 20 {
            patterns.push("bearer_token".to_string());
        }

        // API key pattern (long alphanumeric strings)
        if (line.contains("api_key:") || line.contains("apikey:")) && line.len() > 30 {
            patterns.push("api_key".to_string());
        }

        // AWS pattern
        if line.contains("aws_secret") || line.contains("AKIA") {
            patterns.push("aws_credential".to_string());
        }
    }

    patterns
}

/// Check if compression would lose critical information.
pub fn would_lose_critical_info(original: &str, compressed: &str) -> bool {
    let original_critical = extract_critical_lines(original);
    let compressed_lower = compressed.to_lowercase();

    // Check if all critical lines are still present
    original_critical.iter().any(|critical| {
        !compressed_lower.contains(&critical.to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_auth_data_bearer() {
        assert!(has_auth_data("Authorization: Bearer token123"));
    }

    #[test]
    fn test_has_auth_data_api_key() {
        assert!(has_auth_data("api_key: sk-1234567890"));
    }

    #[test]
    fn test_has_auth_data_aws() {
        assert!(has_auth_data("AWS-Secret-Access-Key: secret123"));
    }

    #[test]
    fn test_has_auth_data_negative() {
        assert!(!has_auth_data("Response: success"));
    }

    #[test]
    fn test_has_critical_errors() {
        assert!(has_critical_errors("Error: connection timeout"));
        assert!(has_critical_errors("Exception: null pointer"));
        assert!(has_critical_errors("FATAL: system down"));
    }

    #[test]
    fn test_has_critical_errors_negative() {
        assert!(!has_critical_errors("Response: ok"));
    }

    #[test]
    fn test_has_tool_definitions() {
        assert!(has_tool_definitions(r#"{"name": "tool", "type": "function"}"#));
        assert!(has_tool_definitions("\"inputSchema\":"));
        assert!(has_tool_definitions("\"properties\":"));
    }

    #[test]
    fn test_has_function_signatures() {
        assert!(has_function_signatures("fn main() {}"));
        assert!(has_function_signatures("pub fn connect()"));
        assert!(has_function_signatures("async fn fetch()"));
        assert!(has_function_signatures("def connect():"));
    }

    #[test]
    fn test_assess_safety_safe() {
        assert_eq!(assess_safety("normal output"), SafetyLevel::Safe);
    }

    #[test]
    fn test_assess_safety_unsafe_auth() {
        assert_eq!(
            assess_safety("Authorization: Bearer secret"),
            SafetyLevel::Unsafe
        );
    }

    #[test]
    fn test_assess_safety_unsafe_tool_def() {
        assert_eq!(
            assess_safety(r#"{"name": "tool", "type": "function"}"#),
            SafetyLevel::Unsafe
        );
    }

    #[test]
    fn test_assess_safety_risky_error() {
        assert_eq!(
            assess_safety("Error: something failed"),
            SafetyLevel::Risky
        );
    }

    #[test]
    fn test_assess_safety_risky_function_sig() {
        assert_eq!(
            assess_safety("fn connect() -> Result"),
            SafetyLevel::Risky
        );
    }

    #[test]
    fn test_extract_critical_lines() {
        let content = "normal line\nError: failed\nmore output\nAuthorization: secret\n";
        let critical = extract_critical_lines(content);
        assert!(critical.iter().any(|l| l.contains("Error")));
        assert!(critical.iter().any(|l| l.contains("Authorization")));
    }

    #[test]
    fn test_validate_no_secrets() {
        let clean = "normal output";
        assert!(validate_no_secrets(clean).is_ok());

        let suspicious = "bearer a1b2c3d4e5f6g7h8i9j0";
        assert!(validate_no_secrets(suspicious).is_err());
    }

    #[test]
    fn test_would_lose_critical_info() {
        let original = "normal text\nError: database failed\nmore text";
        let compressed_good = "normal\nError: database failed\nmore";
        let compressed_bad = "normal\nmore";

        assert!(!would_lose_critical_info(original, compressed_good));
        assert!(would_lose_critical_info(original, compressed_bad));
    }
}
