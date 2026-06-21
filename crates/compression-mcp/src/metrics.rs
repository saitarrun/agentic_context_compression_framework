use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;
use mcp_types::ContentType;

/// Metrics collector for compression operations.
/// Tracks compression effectiveness, accuracy, and workload reduction.
pub struct MetricsCollector {
    total_tokens_saved: Arc<AtomicU64>,
    compressions_count: Arc<AtomicU64>,
    errors_count: Arc<AtomicU64>,
    accuracy_scores: Arc<Mutex<Vec<f64>>>,
    /// Track metrics per content type
    per_type_metrics: Arc<Mutex<HashMap<String, TypeMetrics>>>,
}

#[derive(Debug, Clone)]
struct TypeMetrics {
    count: u64,
    total_original_bytes: u64,
    total_compressed_bytes: u64,
    total_tokens_saved: u64,
    errors: u64,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            total_tokens_saved: Arc::new(AtomicU64::new(0)),
            compressions_count: Arc::new(AtomicU64::new(0)),
            errors_count: Arc::new(AtomicU64::new(0)),
            accuracy_scores: Arc::new(Mutex::new(Vec::new())),
            per_type_metrics: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a successful compression operation.
    pub fn record_compression(&self, tokens_saved: u64) {
        self.total_tokens_saved.fetch_add(tokens_saved, Ordering::Relaxed);
        self.compressions_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record compression with detailed metrics.
    pub fn record_compression_detailed(
        &self,
        tokens_saved: u64,
        original_bytes: usize,
        compressed_bytes: usize,
        content_type: ContentType,
    ) {
        self.record_compression(tokens_saved);

        if let Ok(mut metrics) = self.per_type_metrics.lock() {
            let type_key = format!("{:?}", content_type);
            let entry = metrics.entry(type_key).or_insert(TypeMetrics {
                count: 0,
                total_original_bytes: 0,
                total_compressed_bytes: 0,
                total_tokens_saved: 0,
                errors: 0,
            });

            entry.count += 1;
            entry.total_original_bytes += original_bytes as u64;
            entry.total_compressed_bytes += compressed_bytes as u64;
            entry.total_tokens_saved += tokens_saved;
        }
    }

    /// Record a compression error.
    pub fn record_error(&self) {
        self.errors_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a compression error for a specific content type.
    pub fn record_error_for_type(&self, content_type: ContentType) {
        self.record_error();

        if let Ok(mut metrics) = self.per_type_metrics.lock() {
            let type_key = format!("{:?}", content_type);
            let entry = metrics.entry(type_key).or_insert(TypeMetrics {
                count: 0,
                total_original_bytes: 0,
                total_compressed_bytes: 0,
                total_tokens_saved: 0,
                errors: 0,
            });

            entry.errors += 1;
        }
    }

    /// Record an accuracy score (0.0 - 1.0).
    /// Higher = better; 1.0 = perfect preservation.
    pub fn record_accuracy(&self, score: f64) {
        if (0.0..=1.0).contains(&score) {
            if let Ok(mut scores) = self.accuracy_scores.lock() {
                scores.push(score);
            }
        }
    }

    /// Get current metrics snapshot.
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        let total_saved = self.total_tokens_saved.load(Ordering::Relaxed);
        let count = self.compressions_count.load(Ordering::Relaxed);
        let errors = self.errors_count.load(Ordering::Relaxed);

        let avg_accuracy = if let Ok(scores) = self.accuracy_scores.lock() {
            if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f64>() / scores.len() as f64
            }
        } else {
            0.0
        };

        let success_rate = if count == 0 {
            0.0
        } else {
            ((count - errors) as f64 / count as f64) * 100.0
        };

        let workload_reduction = if count > 0 {
            (total_saved as f64 / (count as f64 * 1000.0)) * 100.0
        } else {
            0.0
        };

        MetricsSnapshot {
            total_tokens_saved,
            compressions_count: count,
            errors_count: errors,
            average_accuracy: avg_accuracy,
            success_rate,
            workload_reduction,
        }
    }

    /// Get per-content-type metrics.
    pub fn get_type_metrics(&self) -> Result<HashMap<String, TypeMetricsSnapshot>, String> {
        if let Ok(metrics) = self.per_type_metrics.lock() {
            Ok(metrics
                .iter()
                .map(|(type_key, m)| {
                    let compression_ratio = if m.total_compressed_bytes > 0 {
                        m.total_original_bytes as f64 / m.total_compressed_bytes as f64
                    } else {
                        1.0
                    };

                    (
                        type_key.clone(),
                        TypeMetricsSnapshot {
                            count: m.count,
                            total_original_bytes: m.total_original_bytes,
                            total_compressed_bytes: m.total_compressed_bytes,
                            compression_ratio,
                            tokens_saved: m.total_tokens_saved,
                            errors: m.errors,
                        },
                    )
                })
                .collect())
        } else {
            Err("Lock error".to_string())
        }
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.total_tokens_saved.store(0, Ordering::Relaxed);
        self.compressions_count.store(0, Ordering::Relaxed);
        self.errors_count.store(0, Ordering::Relaxed);
        if let Ok(mut scores) = self.accuracy_scores.lock() {
            scores.clear();
        }
        if let Ok(mut metrics) = self.per_type_metrics.lock() {
            metrics.clear();
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_tokens_saved: u64,
    pub compressions_count: u64,
    pub errors_count: u64,
    pub average_accuracy: f64,
    pub success_rate: f64,
    pub workload_reduction: f64,
}

#[derive(Debug, Clone)]
pub struct TypeMetricsSnapshot {
    pub count: u64,
    pub total_original_bytes: u64,
    pub total_compressed_bytes: u64,
    pub compression_ratio: f64,
    pub tokens_saved: u64,
    pub errors: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = MetricsCollector::new();
        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_tokens_saved, 0);
        assert_eq!(snapshot.compressions_count, 0);
    }

    #[test]
    fn test_record_compression() {
        let metrics = MetricsCollector::new();
        metrics.record_compression(100);
        metrics.record_compression(50);

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_tokens_saved, 150);
        assert_eq!(snapshot.compressions_count, 2);
    }

    #[test]
    fn test_record_error() {
        let metrics = MetricsCollector::new();
        metrics.record_error();
        metrics.record_error();

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.errors_count, 2);
    }

    #[test]
    fn test_record_accuracy() {
        let metrics = MetricsCollector::new();
        metrics.record_accuracy(0.9);
        metrics.record_accuracy(0.95);
        metrics.record_accuracy(0.85);

        let snapshot = metrics.get_snapshot();
        assert!((snapshot.average_accuracy - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_success_rate_calculation() {
        let metrics = MetricsCollector::new();
        metrics.record_compression(100);
        metrics.record_compression(100);
        metrics.record_error();

        let snapshot = metrics.get_snapshot();
        // 2 successes out of 3 total = 66.67%
        assert!((snapshot.success_rate - 66.67).abs() < 1.0);
    }

    #[test]
    fn test_record_compression_detailed() {
        let metrics = MetricsCollector::new();
        metrics.record_compression_detailed(100, 1000, 100, ContentType::Json);

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_tokens_saved, 100);
        assert_eq!(snapshot.compressions_count, 1);

        let type_metrics = metrics.get_type_metrics().expect("get_type_metrics failed");
        assert!(type_metrics.contains_key("Json"));
        let json_metrics = &type_metrics["Json"];
        assert_eq!(json_metrics.compression_ratio, 10.0);
    }

    #[test]
    fn test_reset() {
        let metrics = MetricsCollector::new();
        metrics.record_compression(100);
        metrics.record_error();
        metrics.record_accuracy(0.9);

        metrics.reset();
        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.total_tokens_saved, 0);
        assert_eq!(snapshot.compressions_count, 0);
        assert_eq!(snapshot.errors_count, 0);
        assert_eq!(snapshot.average_accuracy, 0.0);
    }

    #[test]
    fn test_per_type_metrics() {
        let metrics = MetricsCollector::new();
        metrics.record_compression_detailed(50, 500, 50, ContentType::Json);
        metrics.record_compression_detailed(100, 1000, 50, ContentType::Code);

        let type_metrics = metrics.get_type_metrics().expect("get_type_metrics failed");
        assert_eq!(type_metrics.len(), 2);

        let json = &type_metrics["Json"];
        assert_eq!(json.compression_ratio, 10.0);
        assert_eq!(json.tokens_saved, 50);

        let code = &type_metrics["Code"];
        assert_eq!(code.compression_ratio, 20.0);
        assert_eq!(code.tokens_saved, 100);
    }

    #[test]
    fn test_error_tracking_per_type() {
        let metrics = MetricsCollector::new();
        metrics.record_compression_detailed(50, 500, 50, ContentType::Json);
        metrics.record_error_for_type(ContentType::Json);

        let type_metrics = metrics.get_type_metrics().expect("get_type_metrics failed");
        let json = &type_metrics["Json"];
        assert_eq!(json.errors, 1);
        assert_eq!(json.count, 1);
    }

    #[test]
    fn test_accuracy_bounds() {
        let metrics = MetricsCollector::new();
        metrics.record_accuracy(0.5);
        metrics.record_accuracy(1.5); // Out of bounds, should not be recorded
        metrics.record_accuracy(-0.5); // Out of bounds, should not be recorded
        metrics.record_accuracy(1.0);

        let snapshot = metrics.get_snapshot();
        // Only 0.5 and 1.0 should be recorded
        assert!((snapshot.average_accuracy - 0.75).abs() < 0.01);
    }
}
