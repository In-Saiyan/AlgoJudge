//! C++ language handler

use super::LanguageHandler;

/// Get handler for C++
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "cpp".to_string(),
        source_extension: "cpp".to_string(),
        executable_name: "/workspace/solution".to_string(),
        compile_command: Some(
            "g++ -O2 -std=c++20 -Wall -Wextra -o /workspace/solution /workspace/solution.cpp"
                .to_string(),
        ),
        run_command: "/workspace/solution".to_string(),
    }
}
