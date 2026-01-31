//! Benchmark result models

use serde::{Deserialize, Serialize};

/// Benchmark result for a single test case run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    /// Iteration number (0 = warm-up, discarded)
    pub iteration: u32,
    /// Wall clock time in milliseconds
    pub wall_time_ms: f64,
    /// CPU time in milliseconds
    pub cpu_time_ms: f64,
    /// Peak memory usage in kilobytes
    pub memory_kb: i64,
    /// Whether this was marked as an outlier
    pub is_outlier: bool,
}

/// Aggregated benchmark results across multiple iterations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Number of valid iterations (excluding warm-up)
    pub iterations: u32,

    // Time statistics (milliseconds)
    pub time_avg_ms: f64,
    pub time_median_ms: f64,
    pub time_min_ms: f64,
    pub time_max_ms: f64,
    pub time_stddev_ms: f64,

    // Memory statistics (kilobytes)
    pub memory_avg_kb: i64,
    pub memory_peak_kb: i64,

    /// Individual runs (for detailed analysis)
    pub runs: Vec<BenchmarkRun>,

    /// Outlier information
    pub outliers: Vec<OutlierInfo>,
}

/// Information about an outlier measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierInfo {
    pub iteration: u32,
    pub value_ms: f64,
    pub deviation_percent: f64,
}

impl BenchmarkResult {
    /// Create a new benchmark result from runs
    pub fn from_runs(runs: Vec<BenchmarkRun>) -> Self {
        // Filter out warm-up run (iteration 0) and outliers for stats
        let valid_runs: Vec<&BenchmarkRun> = runs
            .iter()
            .filter(|r| r.iteration > 0 && !r.is_outlier)
            .collect();

        let iterations = valid_runs.len() as u32;

        if iterations == 0 {
            return Self {
                iterations: 0,
                time_avg_ms: 0.0,
                time_median_ms: 0.0,
                time_min_ms: 0.0,
                time_max_ms: 0.0,
                time_stddev_ms: 0.0,
                memory_avg_kb: 0,
                memory_peak_kb: 0,
                runs,
                outliers: vec![],
            };
        }

        // Calculate time statistics
        let times: Vec<f64> = valid_runs.iter().map(|r| r.wall_time_ms).collect();
        let time_avg_ms = times.iter().sum::<f64>() / iterations as f64;
        let time_min_ms = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let time_max_ms = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Calculate median
        let mut sorted_times = times.clone();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let time_median_ms = if iterations % 2 == 0 {
            (sorted_times[iterations as usize / 2 - 1] + sorted_times[iterations as usize / 2])
                / 2.0
        } else {
            sorted_times[iterations as usize / 2]
        };

        // Calculate standard deviation
        let variance = times
            .iter()
            .map(|t| (t - time_avg_ms).powi(2))
            .sum::<f64>()
            / iterations as f64;
        let time_stddev_ms = variance.sqrt();

        // Calculate memory statistics
        let memories: Vec<i64> = valid_runs.iter().map(|r| r.memory_kb).collect();
        let memory_avg_kb = memories.iter().sum::<i64>() / iterations as i64;
        let memory_peak_kb = memories.iter().cloned().max().unwrap_or(0);

        // Find outliers (values > 2 standard deviations from mean)
        let outliers: Vec<OutlierInfo> = runs
            .iter()
            .filter(|r| r.iteration > 0 && r.is_outlier)
            .map(|r| OutlierInfo {
                iteration: r.iteration,
                value_ms: r.wall_time_ms,
                deviation_percent: ((r.wall_time_ms - time_avg_ms) / time_avg_ms) * 100.0,
            })
            .collect();

        Self {
            iterations,
            time_avg_ms,
            time_median_ms,
            time_min_ms,
            time_max_ms,
            time_stddev_ms,
            memory_avg_kb,
            memory_peak_kb,
            runs,
            outliers,
        }
    }

    /// Detect outliers using IQR method
    pub fn detect_outliers(runs: &mut [BenchmarkRun]) {
        if runs.len() < 4 {
            return;
        }

        // Get times excluding warm-up
        let mut times: Vec<f64> = runs
            .iter()
            .filter(|r| r.iteration > 0)
            .map(|r| r.wall_time_ms)
            .collect();

        times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = times.len();
        let q1 = times[n / 4];
        let q3 = times[3 * n / 4];
        let iqr = q3 - q1;

        let lower_bound = q1 - 1.5 * iqr;
        let upper_bound = q3 + 1.5 * iqr;

        // Mark outliers
        for run in runs.iter_mut() {
            if run.iteration > 0 && (run.wall_time_ms < lower_bound || run.wall_time_ms > upper_bound)
            {
                run.is_outlier = true;
            }
        }
    }
}

/// Language-specific benchmark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    /// Language identifier
    pub language: String,
    /// Docker image to use
    pub image: String,
    /// Compilation command (if applicable)
    pub compile_cmd: Option<Vec<String>>,
    /// Run command
    pub run_cmd: Vec<String>,
    /// Source file name
    pub source_file: String,
    /// Compiled output name (if applicable)
    pub output_file: Option<String>,
}
