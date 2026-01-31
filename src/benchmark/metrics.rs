//! Metrics collection and analysis

use std::collections::HashMap;

use crate::models::{BenchmarkResult, BenchmarkRun, OutlierInfo};

/// Metrics collector for benchmark analysis
pub struct MetricsCollector {
    runs: Vec<BenchmarkRun>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self { runs: Vec::new() }
    }

    /// Add a benchmark run
    pub fn add_run(&mut self, run: BenchmarkRun) {
        self.runs.push(run);
    }

    /// Calculate final benchmark results
    pub fn calculate_results(&self) -> Option<BenchmarkResult> {
        if self.runs.is_empty() {
            return None;
        }

        Some(BenchmarkResult::from_runs(self.runs.clone()))
    }

    /// Get number of runs
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Clear all runs
    pub fn clear(&mut self) {
        self.runs.clear();
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance comparison between submissions
#[derive(Debug, Clone)]
pub struct PerformanceComparison {
    pub submission_a: BenchmarkResult,
    pub submission_b: BenchmarkResult,
    pub time_diff_percent: f64,
    pub memory_diff_percent: f64,
    pub faster_submission: char, // 'a', 'b', or 'e' for equal
}

impl PerformanceComparison {
    /// Compare two benchmark results
    pub fn compare(a: &BenchmarkResult, b: &BenchmarkResult) -> Self {
        let time_diff_percent = if a.time_avg_ms > 0.0 {
            ((b.time_avg_ms - a.time_avg_ms) / a.time_avg_ms) * 100.0
        } else {
            0.0
        };

        let memory_diff_percent = if a.memory_avg_kb > 0 {
            ((b.memory_avg_kb - a.memory_avg_kb) as f64 / a.memory_avg_kb as f64) * 100.0
        } else {
            0.0
        };

        let faster_submission = if (a.time_avg_ms - b.time_avg_ms).abs() < 0.001 {
            'e' // Equal
        } else if a.time_avg_ms < b.time_avg_ms {
            'a'
        } else {
            'b'
        };

        Self {
            submission_a: a.clone(),
            submission_b: b.clone(),
            time_diff_percent,
            memory_diff_percent,
            faster_submission,
        }
    }
}

/// Language performance statistics
#[derive(Debug, Clone)]
pub struct LanguageStats {
    pub language: String,
    pub submission_count: i64,
    pub avg_time_ms: f64,
    pub avg_memory_kb: f64,
    pub min_time_ms: f64,
    pub max_time_ms: f64,
}

/// Calculate language statistics for a problem
pub fn calculate_language_stats(
    results: &[(String, BenchmarkResult)],
) -> HashMap<String, LanguageStats> {
    let mut stats: HashMap<String, Vec<&BenchmarkResult>> = HashMap::new();

    for (language, result) in results {
        stats.entry(language.clone()).or_default().push(result);
    }

    stats
        .into_iter()
        .map(|(language, results)| {
            let count = results.len() as i64;
            let avg_time = results.iter().map(|r| r.time_avg_ms).sum::<f64>() / count as f64;
            let avg_memory = results.iter().map(|r| r.memory_avg_kb).sum::<i64>() as f64 / count as f64;
            let min_time = results
                .iter()
                .map(|r| r.time_min_ms)
                .fold(f64::INFINITY, f64::min);
            let max_time = results
                .iter()
                .map(|r| r.time_max_ms)
                .fold(f64::NEG_INFINITY, f64::max);

            (
                language.clone(),
                LanguageStats {
                    language,
                    submission_count: count,
                    avg_time_ms: avg_time,
                    avg_memory_kb: avg_memory,
                    min_time_ms: min_time,
                    max_time_ms: max_time,
                },
            )
        })
        .collect()
}
