//! Zig language handler

use super::LanguageHandler;

/// Get handler for Zig
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "zig".to_string(),
        source_extension: "zig".to_string(),
        executable_name: "/workspace/solution".to_string(),
        compile_command: Some(
            "zig build-exe -O ReleaseFast -femit-bin=/workspace/solution /workspace/solution.zig"
                .to_string(),
        ),
        run_command: "/workspace/solution".to_string(),
    }
}
