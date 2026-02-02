//! Prometheus metrics for Minos

use std::sync::LazyLock;

use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, Opts, Registry, TextEncoder,
};

/// Global metrics registry
pub static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// Execution duration histogram
pub static EXECUTION_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    let opts = HistogramOpts::new(
        "judge_execution_duration_seconds",
        "Time spent executing submissions",
    )
    .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]);

    HistogramVec::new(opts, &["problem_id"]).expect("Failed to create histogram")
});

/// Memory usage histogram
pub static MEMORY_USAGE: LazyLock<HistogramVec> = LazyLock::new(|| {
    let opts = HistogramOpts::new(
        "judge_memory_usage_bytes",
        "Memory used by submissions",
    )
    .buckets(vec![
        1024.0 * 1024.0,        // 1 MB
        16.0 * 1024.0 * 1024.0, // 16 MB
        64.0 * 1024.0 * 1024.0, // 64 MB
        128.0 * 1024.0 * 1024.0, // 128 MB
        256.0 * 1024.0 * 1024.0, // 256 MB
        512.0 * 1024.0 * 1024.0, // 512 MB
        1024.0 * 1024.0 * 1024.0, // 1 GB
    ]);

    HistogramVec::new(opts, &["problem_id"]).expect("Failed to create histogram")
});

/// Verdict counter by type
pub static VERDICT_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    let opts = Opts::new("judge_verdict_total", "Total verdicts by type");
    IntCounterVec::new(opts, &["verdict"]).expect("Failed to create counter")
});

/// Jobs processed counter
pub static JOBS_PROCESSED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new("judge_jobs_processed_total", "Total jobs processed")
        .expect("Failed to create counter")
});

/// Jobs failed counter
pub static JOBS_FAILED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new("judge_jobs_failed_total", "Total jobs that failed")
        .expect("Failed to create counter")
});

/// Currently active jobs gauge
pub static ACTIVE_JOBS: LazyLock<IntGauge> = LazyLock::new(|| {
    IntGauge::new("judge_active_jobs", "Currently active judging jobs")
        .expect("Failed to create gauge")
});

/// Test case generation counter
pub static TESTCASES_GENERATED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "judge_testcases_generated_total",
        "Total test cases generated",
    )
    .expect("Failed to create counter")
});

/// Initialize and register all metrics
pub fn init_metrics() {
    REGISTRY
        .register(Box::new(EXECUTION_DURATION.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(MEMORY_USAGE.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(VERDICT_TOTAL.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(JOBS_PROCESSED.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(JOBS_FAILED.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(ACTIVE_JOBS.clone()))
        .expect("Failed to register metric");
    REGISTRY
        .register(Box::new(TESTCASES_GENERATED.clone()))
        .expect("Failed to register metric");
}

/// Record a verdict
pub fn record_verdict(verdict: &str) {
    VERDICT_TOTAL.with_label_values(&[verdict]).inc();
}

/// Record execution metrics
pub fn record_execution(problem_id: &str, duration_secs: f64, memory_bytes: u64) {
    EXECUTION_DURATION
        .with_label_values(&[problem_id])
        .observe(duration_secs);
    MEMORY_USAGE
        .with_label_values(&[problem_id])
        .observe(memory_bytes as f64);
}

/// HTTP server for Prometheus metrics endpoint
pub struct MetricsServer;

impl MetricsServer {
    /// Run the metrics server
    pub async fn run(port: u16) -> anyhow::Result<()> {
        use axum::{routing::get, Router};
        use std::net::SocketAddr;

        // Initialize metrics
        init_metrics();

        let app = Router::new()
            .route("/metrics", get(Self::metrics_handler))
            .route("/health", get(|| async { "OK" }));

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        tracing::info!("Metrics server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    async fn metrics_handler() -> String {
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}
