//! Rust language handler

use super::LanguageHandler;

/// Get handler for Rust
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "rust".to_string(),
        source_extension: "rs".to_string(),
        executable_name: "/workspace/solution".to_string(),
        compile_command: Some(
            "rustc -O -o /workspace/solution /workspace/solution.rs".to_string(),
        ),
        run_command: "/workspace/solution".to_string(),
    }
}
