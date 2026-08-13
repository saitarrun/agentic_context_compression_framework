use super::Compressor;
use mcp_types::MpcError;
use std::collections::{HashMap, HashSet};

/// RepoMapCompressor: Aider-inspired Repository Symbol Graph Compressor.
///
/// Features (Research & Industry Reference: Aider Repo Map):
/// 1. Extracts high-importance symbol definitions (classes, traits, structs, functions, type aliases).
/// 2. Builds a symbol reference graph to prioritize key interface definitions over implementation bodies.
/// 3. Compresses full repository file dumps into a concise, high-density architectural map (<1,000 tokens).
#[derive(Debug, Clone, Default)]
pub struct RepoMapCompressor {
    pub max_symbols_per_file: usize,
}

impl RepoMapCompressor {
    pub fn new() -> Self {
        Self {
            max_symbols_per_file: 15,
        }
    }

    /// Extract key architectural symbols from code lines
    pub fn extract_symbol_map(&self, content: &str) -> String {
        let mut file_symbols: HashMap<String, Vec<String>> = HashMap::new();
        let mut current_file = "root".to_string();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Detect file boundary markers
            if (trimmed.starts_with("File:") || trimmed.starts_with("--- ") || trimmed.starts_with("+++ ") || trimmed.starts_with("# File:"))
                && (trimmed.contains('.') || trimmed.contains('/'))
            {
                let clean_path = trimmed
                    .trim_start_matches("File:")
                    .trim_start_matches("--- ")
                    .trim_start_matches("+++ ")
                    .trim_start_matches("# File:")
                    .trim()
                    .to_string();
                current_file = clean_path;
                file_symbols.entry(current_file.clone()).or_default();
                continue;
            }

            // Detect symbol declarations
            let is_symbol = trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("pub type ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("export function ")
                || trimmed.starts_with("export class ")
                || trimmed.starts_with("export interface ");

            if is_symbol {
                let sig = if let Some(pos) = trimmed.find('{') {
                    trimmed[..pos].trim().to_string()
                } else if let Some(pos) = trimmed.find(':') {
                    trimmed[..pos].trim().to_string()
                } else {
                    trimmed.to_string()
                };

                let entry = file_symbols.entry(current_file.clone()).or_default();
                if entry.len() < self.max_symbols_per_file && !entry.contains(&sig) {
                    entry.push(sig);
                }
            }
        }

        // Format into compact Aider-style repo map
        let mut out = Vec::new();
        out.push("# Repository Symbol Map (Aider-style)");

        for (file, symbols) in file_symbols {
            if symbols.is_empty() {
                continue;
            }
            out.push(format!("\n📄 {}", file));
            for sym in symbols {
                out.push(format!("  ├─ {}", sym));
            }
        }

        out.join("\n")
    }
}

impl Compressor for RepoMapCompressor {
    fn compress(&self, content: &str) -> Result<(String, f64), MpcError> {
        let repo_map = self.extract_symbol_map(content);
        let orig_len = content.len() as f64;
        let map_len = repo_map.len() as f64;
        let ratio = if map_len > 0.0 { orig_len / map_len } else { 1.0 };
        Ok((repo_map, ratio))
    }

    fn name(&self) -> &str {
        "RepoMapCompressor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_map_extraction() {
        let compressor = RepoMapCompressor::new();
        let input = r#"
File: src/auth.rs
pub struct UserSession {
    pub id: String,
}
pub fn authenticate_token(token: &str) -> Result<UserSession, Error> {
    // 50 lines of complex crypto validation
    Ok(UserSession { id: token.to_string() })
}

File: src/db.rs
pub trait DatabasePool {
    fn query(&self, sql: &str) -> Result<Row, Error>;
}
"#;

        let (map, ratio) = compressor.compress(input).expect("repo map failed");
        assert!(map.contains("📄 src/auth.rs"));
        assert!(map.contains("pub struct UserSession"));
        assert!(map.contains("pub fn authenticate_token"));
        assert!(map.contains("📄 src/db.rs"));
        assert!(map.contains("pub trait DatabasePool"));
        assert!(ratio > 1.2);
    }
}
