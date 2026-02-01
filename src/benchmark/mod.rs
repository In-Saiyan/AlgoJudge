//! Benchmark execution engine
//!
//! AlgoJudge supports two types of benchmarking:
//! 
//! 1. **Legacy Runner** (`runner.rs`): Traditional competitive programming style
//!    with static test cases and direct source code submission.
//!
//! 2. **Algo Runner** (`algo_runner.rs`): Advanced algorithmic benchmarking for
//!    large-scale problems (e.g., sorting 4GB files) using:
//!    - ZIP submissions with compile.sh and run.sh
//!    - Dynamic test case generation
//!    - Custom output verification

pub mod algo_runner;
pub mod container;
pub mod languages;
pub mod metrics;
pub mod runner;

pub use algo_runner::AlgoBenchmarkRunner;
pub use container::ContainerManager;
pub use metrics::MetricsCollector;
pub use runner::BenchmarkRunner;
