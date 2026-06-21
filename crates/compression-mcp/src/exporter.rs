/// Metrics exporter for compression statistics
/// Exports to multiple formats (CSV, Prometheus, JSON)

use crate::metrics::MetricsSnapshot;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Metrics exporter for various output formats
pub struct MetricsExporter;

impl MetricsExporter {
    /// Export metrics to CSV file
    pub fn export_to_csv<P: AsRef<Path>>(
        snapshot: &MetricsSnapshot,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::create(path)?;

        writeln!(file, "metric,value,unit")?;
        writeln!(file, "total_tokens_saved,{},tokens", snapshot.total_tokens_saved)?;
        writeln!(file, "compressions_count,{},count", snapshot.compressions_count)?;
        writeln!(file, "errors_count,{},count", snapshot.errors_count)?;
        writeln!(
            file,
            "average_accuracy,{:.4},ratio (0-1)",
            snapshot.average_accuracy
        )?;
        writeln!(
            file,
            "success_rate,{:.2},percentage",
            snapshot.success_rate
        )?;
        writeln!(
            file,
            "workload_reduction,{:.2},percentage",
            snapshot.workload_reduction
        )?;

        Ok(())
    }

    /// Export metrics to Prometheus format
    pub fn export_to_prometheus(snapshot: &MetricsSnapshot) -> String {
        format!(
            r#"# HELP headroom_tokens_saved_total Cumulative tokens saved via compression
# TYPE headroom_tokens_saved_total counter
headroom_tokens_saved_total {{}} {}

# HELP headroom_compressions_total Total compression operations
# TYPE headroom_compressions_total counter
headroom_compressions_total {{}} {}

# HELP headroom_errors_total Total compression errors
# TYPE headroom_errors_total counter
headroom_errors_total {{}} {}

# HELP headroom_compression_accuracy_gauge Average compression accuracy (0-1)
# TYPE headroom_compression_accuracy_gauge gauge
headroom_compression_accuracy_gauge {{}} {:.4}

# HELP headroom_compression_success_rate Compression success rate (percentage)
# TYPE headroom_compression_success_rate gauge
headroom_compression_success_rate {{}} {:.2}

# HELP headroom_workload_reduction_percent Workload reduction percentage
# TYPE headroom_workload_reduction_percent gauge
headroom_workload_reduction_percent {{}} {:.2}
"#,
            snapshot.total_tokens_saved,
            snapshot.compressions_count,
            snapshot.errors_count,
            snapshot.average_accuracy,
            snapshot.success_rate,
            snapshot.workload_reduction
        )
    }

    /// Export metrics to JSON format
    pub fn export_to_json(snapshot: &MetricsSnapshot) -> String {
        let json = serde_json::json!({
            "metrics": {
                "total_tokens_saved": snapshot.total_tokens_saved,
                "compressions_count": snapshot.compressions_count,
                "errors_count": snapshot.errors_count,
                "average_accuracy": snapshot.average_accuracy,
                "success_rate": snapshot.success_rate,
                "workload_reduction": snapshot.workload_reduction,
            },
            "metadata": {
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "version": env!("CARGO_PKG_VERSION"),
            }
        });

        serde_json::to_string_pretty(&json).unwrap_or_default()
    }

    /// Export metrics in detailed analytics format (for reports)
    pub fn export_to_analytics(snapshot: &MetricsSnapshot) -> String {
        let cost_per_token = 0.000025;  // $0.025 per 1K tokens
        let total_cost_saved = snapshot.total_tokens_saved as f64 * cost_per_token;
        let tokens_per_operation = if snapshot.compressions_count > 0 {
            snapshot.total_tokens_saved as f64 / snapshot.compressions_count as f64
        } else {
            0.0
        };

        // Format numbers with thousand separators
        let format_number = |n: u64| {
            let s = n.to_string();
            let mut result = String::new();
            let mut count = 0;
            for c in s.chars().rev() {
                if count > 0 && count % 3 == 0 {
                    result.insert(0, ',');
                }
                result.insert(0, c);
                count += 1;
            }
            result
        };
        let tokens_saved_fmt = format_number(snapshot.total_tokens_saved);
        let compressions_fmt = format_number(snapshot.compressions_count);
        let errors_fmt = format_number(snapshot.errors_count);

        format!(
            r#"# Compression Metrics Report

## Summary
- **Tokens Saved:** {}
- **Successful Compressions:** {}
- **Compression Errors:** {}
- **Success Rate:** {:.2}%
- **Average Compression Accuracy:** {:.2}%

## Cost Impact
- **Cost Saved (@ $0.025/1K tokens):** ${:.2}
- **Avg Tokens Saved per Operation:** {:.0}
- **Workload Reduction:** {:.2}%

## Performance
- **Tokens per successful compression:** {:.0}
- **Error rate:** {:.2}%
- **Quality:** Average accuracy {:.2}%

## Insights
{}
"#,
            tokens_saved_fmt,
            compressions_fmt,
            errors_fmt,
            snapshot.success_rate,
            snapshot.average_accuracy * 100.0,
            total_cost_saved,
            tokens_per_operation,
            snapshot.workload_reduction,
            tokens_per_operation,
            (snapshot.errors_count as f64 / snapshot.compressions_count.max(1) as f64) * 100.0,
            snapshot.average_accuracy * 100.0,
            Self::generate_insights(snapshot)
        )
    }

    /// Generate human-readable insights from metrics
    fn generate_insights(snapshot: &MetricsSnapshot) -> String {
        let mut insights = Vec::new();

        // Token savings insight
        if snapshot.total_tokens_saved >= 100000 {
            insights.push("✓ Exceptional token savings: >100K tokens saved".to_string());
        } else if snapshot.total_tokens_saved > 50000 {
            insights.push("✓ Strong token savings: >50K tokens saved".to_string());
        } else if snapshot.total_tokens_saved > 10000 {
            insights.push("✓ Moderate token savings: >10K tokens saved".to_string());
        }

        // Success rate insight
        if snapshot.success_rate > 95.0 {
            insights.push("✓ Excellent reliability: >95% success rate".to_string());
        } else if snapshot.success_rate < 90.0 {
            insights.push("⚠ Consider investigation: <90% success rate".to_string());
        }

        // Accuracy insight
        if snapshot.average_accuracy > 0.95 {
            insights.push("✓ High accuracy maintained: >95% preservation".to_string());
        } else if snapshot.average_accuracy < 0.90 {
            insights.push("⚠ Accuracy degradation detected: <90%".to_string());
        }

        // Workload insight
        if snapshot.workload_reduction > 40.0 {
            insights.push("✓ Strong workload reduction: >40%".to_string());
        } else if snapshot.workload_reduction < 20.0 {
            insights.push("⚠ Limited workload reduction: <20%".to_string());
        }

        if insights.is_empty() {
            insights.push("Metrics within normal ranges".to_string());
        }

        insights.join("\n- ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_to_prometheus() {
        let snapshot = MetricsSnapshot {
            total_tokens_saved: 1000,
            compressions_count: 10,
            errors_count: 1,
            average_accuracy: 0.95,
            success_rate: 90.0,
            workload_reduction: 12.5,
        };

        let output = MetricsExporter::export_to_prometheus(&snapshot);
        assert!(output.contains("headroom_tokens_saved_total"));
        assert!(output.contains("1000"));
        assert!(output.contains("10"));
        assert!(output.contains("0.9500"));
    }

    #[test]
    fn test_export_to_json() {
        let snapshot = MetricsSnapshot {
            total_tokens_saved: 1000,
            compressions_count: 10,
            errors_count: 1,
            average_accuracy: 0.95,
            success_rate: 90.0,
            workload_reduction: 12.5,
        };

        let output = MetricsExporter::export_to_json(&snapshot);
        assert!(output.contains("total_tokens_saved"));
        assert!(output.contains("1000"));
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["metrics"]["total_tokens_saved"], 1000);
    }

    #[test]
    fn test_export_to_csv() {
        let snapshot = MetricsSnapshot {
            total_tokens_saved: 1000,
            compressions_count: 10,
            errors_count: 1,
            average_accuracy: 0.95,
            success_rate: 90.0,
            workload_reduction: 12.5,
        };

        let file = NamedTempFile::new().expect("Failed to create temp file");
        let path = file.path();

        let result = MetricsExporter::export_to_csv(&snapshot, path);
        assert!(result.is_ok());

        // Read and verify
        let content = std::fs::read_to_string(path).expect("Failed to read file");
        assert!(content.contains("total_tokens_saved"));
        assert!(content.contains("1000"));
    }

    #[test]
    fn test_export_to_analytics() {
        let snapshot = MetricsSnapshot {
            total_tokens_saved: 100000,
            compressions_count: 50,
            errors_count: 2,
            average_accuracy: 0.98,
            success_rate: 96.0,
            workload_reduction: 45.0,
        };

        let output = MetricsExporter::export_to_analytics(&snapshot);
        assert!(output.contains("100,000"));
        assert!(output.contains("Exceptional token savings"));
        assert!(output.contains("Excellent reliability"));
    }

    #[test]
    fn test_insights_generation() {
        // Strong metrics
        let strong = MetricsSnapshot {
            total_tokens_saved: 100000,
            compressions_count: 100,
            errors_count: 2,
            average_accuracy: 0.98,
            success_rate: 98.0,
            workload_reduction: 50.0,
        };

        let insights = MetricsExporter::export_to_analytics(&strong);
        assert!(insights.contains("Exceptional"));
        assert!(insights.contains("Excellent reliability"));
    }
}
