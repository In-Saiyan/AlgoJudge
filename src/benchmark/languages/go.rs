//! Go language handler

use super::LanguageHandler;

/// Get handler for Go
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "go".to_string(),
        source_extension: "go".to_string(),
        executable_name: "/workspace/solution".to_string(),
        compile_command: Some(
            "go build -o /workspace/solution /workspace/solution.go".to_string(),
        ),
        run_command: "/workspace/solution".to_string(),
    }
}
