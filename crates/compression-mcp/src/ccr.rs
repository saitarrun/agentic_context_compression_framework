use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use mcp_types::SearchMatch;

/// Compression Control Record: reversible compression backend with granular BM25/keyword search.
/// Stores original outputs for retrieval on demand, enabling agents to:
/// 1. Retrieve the entire original output with `headroom_retrieve(id)`.
/// 2. Search and retrieve only relevant snippet lines with `headroom_search(id, query, limit)`.
#[derive(Debug, Clone)]
pub struct CcrRecord {
    pub id: String,
    pub original: String,
    pub timestamp: u64,
    pub original_size: usize,
    pub compressed_size: Option<usize>,
}

pub struct CcrBackend {
    storage: Arc<Mutex<HashMap<String, CcrRecord>>>,
    max_entries: usize,
}

impl CcrBackend {
    /// Create a new CCR backend with default settings.
    pub fn new() -> Self {
        Self::with_capacity(10000)
    }

    /// Create with custom capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
            max_entries,
        }
    }

    /// Store the original output and return an ID.
    pub fn store(&self, original: String) -> Result<String, String> {
        self.store_with_compressed_size(&original, None)
    }

    /// Store original with compressed size metadata.
    pub fn store_with_compressed_size(
        &self,
        original: &str,
        compressed_size: Option<usize>,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = current_timestamp();
        let original_size = original.len();

        let record = CcrRecord {
            id: id.clone(),
            original: original.to_string(),
            timestamp,
            original_size,
            compressed_size,
        };

        let mut storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        if storage.len() >= self.max_entries {
            self.evict_oldest(&mut storage)?;
        }

        storage.insert(id.clone(), record);
        Ok(id)
    }

    /// Retrieve the original output by ID (byte-equal to original).
    pub fn retrieve(&self, id: &str) -> Result<String, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        storage
            .get(id)
            .map(|r| r.original.clone())
            .ok_or_else(|| format!("No stored output with ID: {}", id))
    }

    /// Perform sub-observation BM25 / keyword search over stored uncompressed output
    pub fn search(&self, id: &str, query: &str, max_results: usize) -> Result<Vec<SearchMatch>, String> {
        let raw = self.retrieve(id)?;
        let query_terms: Vec<String> = query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() > 1)
            .collect();

        if query_terms.is_empty() {
            return Ok(Vec::new());
        }

        let mut matches = Vec::new();
        let lines: Vec<&str> = raw.lines().collect();

        for (idx, line) in lines.iter().enumerate() {
            let line_lower = line.to_lowercase();
            let mut matched_terms = 0;
            let mut term_score = 0.0;

            for term in &query_terms {
                let occurrences = line_lower.matches(term.as_str()).count();
                if occurrences > 0 {
                    matched_terms += 1;
                    term_score += occurrences as f64 * (1.0 / (term.len() as f64).sqrt().max(1.0));
                }
            }

            if matched_terms > 0 {
                // Boost score if all terms match
                let coverage = matched_terms as f64 / query_terms.len() as f64;
                let final_score = term_score * (1.0 + coverage * 2.0);

                matches.push(SearchMatch {
                    line_number: idx + 1,
                    content: line.to_string(),
                    score: final_score,
                });
            }
        }

        // Sort descending by score
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(max_results);

        Ok(matches)
    }

    /// Retrieve record with metadata.
    pub fn retrieve_record(&self, id: &str) -> Result<CcrRecord, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        storage
            .get(id)
            .cloned()
            .ok_or_else(|| format!("No stored record with ID: {}", id))
    }

    /// Delete a stored output.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        storage.remove(id);
        Ok(())
    }

    /// Get the number of stored entries.
    pub fn count(&self) -> Result<usize, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(storage.len())
    }

    /// Get total storage size in bytes.
    pub fn total_size(&self) -> Result<usize, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        Ok(storage.values().map(|r| r.original_size).sum())
    }

    /// Get storage statistics.
    pub fn stats(&self) -> Result<CcrStats, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;

        let count = storage.len();
        let total_original_size: usize = storage.iter().map(|(_, r)| r.original_size).sum();
        let total_compressed_size: usize = storage
            .iter()
            .filter_map(|(_, r)| r.compressed_size)
            .sum();

        let compression_ratio = if total_compressed_size > 0 {
            total_original_size as f64 / total_compressed_size as f64
        } else {
            1.0
        };

        Ok(CcrStats {
            stored_records: count,
            total_original_bytes: total_original_size,
            total_compressed_bytes: total_compressed_size,
            average_compression_ratio: compression_ratio,
        })
    }

    /// Clear all stored data.
    pub fn clear(&self) -> Result<(), String> {
        let mut storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        storage.clear();
        Ok(())
    }

    /// Evict oldest record (by timestamp).
    fn evict_oldest(&self, storage: &mut HashMap<String, CcrRecord>) -> Result<(), String> {
        if let Some((oldest_id, _)) = storage
            .iter()
            .min_by_key(|(_, r)| r.timestamp)
            .map(|(id, r)| (id.clone(), r.clone()))
        {
            storage.remove(&oldest_id);
        }
        Ok(())
    }

    /// List all stored IDs.
    pub fn list_ids(&self) -> Result<Vec<String>, String> {
        let storage = self
            .storage
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        Ok(storage.keys().cloned().collect())
    }
}

impl Default for CcrBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct CcrStats {
    pub stored_records: usize,
    pub total_original_bytes: usize,
    pub total_compressed_bytes: usize,
    pub average_compression_ratio: f64,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ccr_search() {
        let ccr = CcrBackend::new();
        let content = "Starting server at 127.0.0.1:8080\nDatabase pool initialized\nError: connection timeout in handler at connection.rs:42\nServer listening";
        let id = ccr.store(content.to_string()).expect("store failed");

        let matches = ccr.search(&id, "connection timeout", 5).expect("search failed");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line_number, 3);
        assert!(matches[0].content.contains("Error: connection timeout"));
    }

    #[test]
    fn test_ccr_store_and_retrieve() {
        let ccr = CcrBackend::new();
        let original = "original output";
        let id = ccr.store(original.to_string()).expect("store failed");
        let retrieved = ccr.retrieve(&id).expect("retrieve failed");
        assert_eq!(retrieved, original);
    }
}
