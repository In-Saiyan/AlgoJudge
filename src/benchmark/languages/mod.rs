//! Language-specific handlers for compilation and execution

pub mod c;
pub mod cpp;
pub mod go;
pub mod python;
pub mod rust;
pub mod zig;

use crate::{constants, error::AppResult};

/// Language handler for compilation and execution
#[derive(Debug, Clone)]
pub struct LanguageHandler {
    language: String,
    source_extension: String,
    executable_name: String,
    compile_command: Option<String>,
    run_command: String,
}

impl LanguageHandler {
    /// Get handler for a specific language
    pub fn for_language(language: &str) -> AppResult<Self> {
        match language {
            constants::languages::C => Ok(c::handler()),
            constants::languages::CPP => Ok(cpp::handler()),
            constants::languages::RUST => Ok(rust::handler()),
            constants::languages::GO => Ok(go::handler()),
            constants::languages::ZIG => Ok(zig::handler()),
            constants::languages::PYTHON => Ok(python::handler()),
            _ => Err(anyhow::anyhow!("Unsupported language: {}", language).into()),
        }
    }

    /// Get the source file name
    pub fn source_file(&self) -> String {
        format!("solution.{}", self.source_extension)
    }

    /// Get the compile command (if needed)
    pub fn compile_command(&self) -> Option<String> {
        self.compile_command.clone()
    }

    /// Get the executable path
    pub fn executable(&self) -> String {
        self.executable_name.clone()
    }

    /// Get the run command
    pub fn run_command(&self) -> String {
        self.run_command.clone()
    }
}
