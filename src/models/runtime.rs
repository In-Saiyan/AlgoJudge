//! Runtime environment model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Runtime environment for running submissions
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Runtime {
    pub id: Uuid,
    /// Unique name (e.g., "cpp", "rust", "python")
    pub name: String,
    /// Display name (e.g., "C++ (G++ 13, C++20)")
    pub display_name: String,
    /// Docker image to use
    pub docker_image: String,
    /// Default compile command template
    /// Use {source} and {output} as placeholders
    pub default_compile_cmd: Option<String>,
    /// Default run command template
    pub default_run_cmd: Option<String>,
    /// Whether this runtime is active
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl Runtime {
    /// Build compile command from template
    pub fn compile_command(&self, source: &str, output: &str) -> Option<String> {
        self.default_compile_cmd.as_ref().map(|cmd| {
            cmd.replace("{source}", source)
               .replace("{output}", output)
        })
    }
    
    /// Build run command from template
    pub fn run_command(&self, binary: &str) -> String {
        self.default_run_cmd
            .as_ref()
            .map(|cmd| cmd.replace("{output}", binary))
            .unwrap_or_else(|| format!("./{}", binary))
    }
    
    /// Check if this runtime requires compilation
    pub fn requires_compilation(&self) -> bool {
        self.default_compile_cmd.is_some()
    }
}
