//! C language handler

use super::LanguageHandler;

/// Get handler for C
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "c".to_string(),
        source_extension: "c".to_string(),
        executable_name: "/workspace/solution".to_string(),
        compile_command: Some(
            "gcc -O2 -std=c17 -Wall -Wextra -o /workspace/solution /workspace/solution.c -lm"
                .to_string(),
        ),
        run_command: "/workspace/solution".to_string(),
    }
}
