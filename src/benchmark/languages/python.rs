//! Python language handler

use super::LanguageHandler;

/// Get handler for Python
pub fn handler() -> LanguageHandler {
    LanguageHandler {
        language: "python".to_string(),
        source_extension: "py".to_string(),
        executable_name: "/workspace/solution.py".to_string(),
        compile_command: Some(
            // Syntax check only
            "python3 -m py_compile /workspace/solution.py".to_string(),
        ),
        run_command: "python3 /workspace/solution.py".to_string(),
    }
}
