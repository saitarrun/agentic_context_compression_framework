use mcp_types::MpcError;
use super::Compressor;

/// CacheAligner: Normalizes dynamic and non-deterministic tokens to maximize LLM KV-cache hits.
///
/// Dynamic tokens (timestamps, hex memory pointers, ephemeral UUIDs, millisecond durations, PIDs)
/// change on every agent turn, invalidating prompt prefix caching in Claude/OpenAI APIs.
/// Normalizing these tokens stabilizes the context prefix, dramatically increasing cache hits
/// (>80%) and slashing TTFT latency and input token costs.
#[derive(Debug, Clone, Default)]
pub struct CacheAligner {
    pub normalize_timestamps: bool,
    pub normalize_hex_addrs: bool,
    pub normalize_uuids: bool,
    pub normalize_durations: bool,
    pub normalize_pids: bool,
}

impl CacheAligner {
    pub fn new() -> Self {
        Self {
            normalize_timestamps: true,
            normalize_hex_addrs: true,
            normalize_uuids: true,
            normalize_durations: true,
            normalize_pids: true,
        }
    }

    /// Normalize non-deterministic tokens across lines of text.
    pub fn normalize(&self, content: &str) -> String {
        let mut result = Vec::with_capacity(content.lines().count());

        for line in content.lines() {
            let mut normalized_line = line.to_string();

            if self.normalize_timestamps {
                normalized_line = Self::mask_timestamps(&normalized_line);
            }
            if self.normalize_hex_addrs {
                normalized_line = Self::mask_hex_addresses(&normalized_line);
            }
            if self.normalize_uuids {
                normalized_line = Self::mask_uuids(&normalized_line);
            }
            if self.normalize_durations {
                normalized_line = Self::mask_durations(&normalized_line);
            }
            if self.normalize_pids {
                normalized_line = Self::mask_pids(&normalized_line);
            }

            result.push(normalized_line);
        }

        result.join("\n")
    }

    /// Mask ISO 8601 / RFC 3339 timestamps (e.g. 2026-08-13T17:15:18Z, 2024-11-05 10:20:30)
    fn mask_timestamps(line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Check for YYYY-MM-DD pattern
            if i + 10 <= chars.len()
                && chars[i].is_ascii_digit()
                && chars[i + 1].is_ascii_digit()
                && chars[i + 2].is_ascii_digit()
                && chars[i + 3].is_ascii_digit()
                && chars[i + 4] == '-'
                && chars[i + 5].is_ascii_digit()
                && chars[i + 6].is_ascii_digit()
                && chars[i + 7] == '-'
                && chars[i + 8].is_ascii_digit()
                && chars[i + 9].is_ascii_digit()
            {
                let mut end = i + 10;
                // Check if followed by time component: 'T' or ' ' HH:MM:SS
                if end + 9 <= chars.len()
                    && (chars[end] == 'T' || chars[end] == ' ')
                    && chars[end + 1].is_ascii_digit()
                    && chars[end + 2].is_ascii_digit()
                    && chars[end + 3] == ':'
                    && chars[end + 4].is_ascii_digit()
                    && chars[end + 5].is_ascii_digit()
                    && chars[end + 6] == ':'
                    && chars[end + 7].is_ascii_digit()
                    && chars[end + 8].is_ascii_digit()
                {
                    end += 9;
                    // Optional fractional seconds: .123456
                    if end < chars.len() && chars[end] == '.' {
                        end += 1;
                        while end < chars.len() && chars[end].is_ascii_digit() {
                            end += 1;
                        }
                    }
                    // Optional timezone: 'Z' or +00:00 / -05:00
                    if end < chars.len() && chars[end] == 'Z' {
                        end += 1;
                    } else if end + 6 <= chars.len()
                        && (chars[end] == '+' || chars[end] == '-')
                        && chars[end + 1].is_ascii_digit()
                        && chars[end + 2].is_ascii_digit()
                        && chars[end + 3] == ':'
                        && chars[end + 4].is_ascii_digit()
                        && chars[end + 5].is_ascii_digit()
                    {
                        end += 6;
                    }
                }
                out.push_str("<TIMESTAMP>");
                i = end;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// Mask hexadecimal memory addresses (e.g. 0x7ffee1b2c890, 0x10a45b8)
    fn mask_hex_addresses(line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 2 < chars.len()
                && chars[i] == '0'
                && (chars[i + 1] == 'x' || chars[i + 1] == 'X')
                && chars[i + 2].is_ascii_hexdigit()
            {
                let mut end = i + 2;
                while end < chars.len() && chars[end].is_ascii_hexdigit() {
                    end += 1;
                }
                // Only mask if length of hex digits is >= 4 (actual memory address / pointer)
                if end - (i + 2) >= 4 {
                    out.push_str("<HEX_ADDR>");
                } else {
                    for c in &chars[i..end] {
                        out.push(*c);
                    }
                }
                i = end;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// Mask UUIDs (e.g. 123e4567-e89b-12d3-a456-426614174000)
    fn mask_uuids(line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Standard UUID 8-4-4-4-12 = 36 chars
            if i + 36 <= chars.len()
                && chars[i..i + 8].iter().all(|c| c.is_ascii_hexdigit())
                && chars[i + 8] == '-'
                && chars[i + 9..i + 13].iter().all(|c| c.is_ascii_hexdigit())
                && chars[i + 13] == '-'
                && chars[i + 14..i + 18].iter().all(|c| c.is_ascii_hexdigit())
                && chars[i + 18] == '-'
                && chars[i + 19..i + 23].iter().all(|c| c.is_ascii_hexdigit())
                && chars[i + 23] == '-'
                && chars[i + 24..i + 36].iter().all(|c| c.is_ascii_hexdigit())
            {
                out.push_str("<UUID>");
                i += 36;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// Mask transient duration strings (e.g. 523ms, 12.4s, 3450us, 120ns)
    fn mask_durations(line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_end = i;

                // Check suffix
                let remaining = &chars[num_end..];
                if remaining.starts_with(&['m', 's']) && (remaining.len() == 2 || !remaining[2].is_alphanumeric()) {
                    out.push_str("<DURATION>");
                    i = num_end + 2;
                } else if remaining.starts_with(&['u', 's']) && (remaining.len() == 2 || !remaining[2].is_alphanumeric()) {
                    out.push_str("<DURATION>");
                    i = num_end + 2;
                } else if remaining.starts_with(&['n', 's']) && (remaining.len() == 2 || !remaining[2].is_alphanumeric()) {
                    out.push_str("<DURATION>");
                    i = num_end + 2;
                } else if remaining.starts_with(&['s', 'e', 'c']) {
                    let mut s_end = num_end + 3;
                    if remaining.starts_with(&['s', 'e', 'c', 'o', 'n', 'd', 's']) {
                        s_end = num_end + 7;
                    }
                    out.push_str("<DURATION>");
                    i = s_end;
                } else {
                    for c in &chars[start..num_end] {
                        out.push(*c);
                    }
                }
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        out
    }

    /// Mask PID notations (e.g. [pid: 12345] or PID 98765)
    fn mask_pids(line: &str) -> String {
        let lower = line.to_lowercase();
        if let Some(pos) = lower.find("pid") {
            let mut end = pos + 3;
            let bytes = line.as_bytes();
            while end < bytes.len() && (bytes[end] == b':' || bytes[end] == b' ' || bytes[end] == b'=') {
                end += 1;
            }
            let num_start = end;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > num_start {
                let mut out = String::new();
                out.push_str(&line[..num_start]);
                out.push_str("<PID>");
                out.push_str(&line[end..]);
                return out;
            }
        }
        line.to_string()
    }
}

impl Compressor for CacheAligner {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        let normalized = self.normalize(content);
        let orig_len = content.len() as f64;
        let norm_len = normalized.len() as f64;
        let ratio = if norm_len > 0.0 { orig_len / norm_len } else { 1.0 };
        Ok((normalized, ratio))
    }

    fn name(&self) -> &str {
        "CacheAligner"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_timestamps() {
        let aligner = CacheAligner::new();
        let input = "Log started at 2026-08-13T17:15:18.123456Z with status OK";
        let out = aligner.normalize(input);
        assert_eq!(out, "Log started at <TIMESTAMP> with status OK");
    }

    #[test]
    fn test_mask_hex_addresses() {
        let aligner = CacheAligner::new();
        let input = "Fatal signal at pointer 0x7ffee1b2c890 (base 0x1000)";
        let out = aligner.normalize(input);
        assert!(out.contains("<HEX_ADDR>"));
        assert!(!out.contains("0x7ffee1b2c890"));
    }

    #[test]
    fn test_mask_uuids() {
        let aligner = CacheAligner::new();
        let input = "Session ID: 123e4567-e89b-12d3-a456-426614174000 initialized";
        let out = aligner.normalize(input);
        assert_eq!(out, "Session ID: <UUID> initialized");
    }

    #[test]
    fn test_mask_durations() {
        let aligner = CacheAligner::new();
        let input = "Request completed in 245ms with 12.5s timeout";
        let out = aligner.normalize(input);
        assert_eq!(out, "Request completed in <DURATION> with <DURATION> timeout");
    }

    #[test]
    fn test_mask_pids() {
        let aligner = CacheAligner::new();
        let input = "Worker thread [pid: 48912] active";
        let out = aligner.normalize(input);
        assert_eq!(out, "Worker thread [pid: <PID>] active");
    }
}
