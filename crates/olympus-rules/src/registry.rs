//! SpecRegistry for dynamic rule building from JSON configuration.
//!
//! This module allows rules to be constructed at runtime from serialized
//! JSON configurations, enabling admin dashboard rule management.

use crate::config::RuleConfig;
use crate::specification::{AllOf, AnyOf, BoxedSpec, Not, Specification};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Factory function type for creating specifications from parameters.
pub type SpecFactory<Ctx> = Arc<dyn Fn(&HashMap<String, Value>) -> Option<BoxedSpec<Ctx>> + Send + Sync>;

/// Registry for dynamically building specifications from configuration.
///
/// The registry maps specification names to factory functions that can
/// create instances with optional parameters.
///
/// # Example
///
/// ```ignore
/// use olympus_rules::registry::SpecRegistry;
/// use olympus_rules::rules::*;
///
/// let mut registry = SpecRegistry::<FileContext>::new();
///
/// // Register specs
/// registry.register("LastAccessOlderThan", |params| {
///     let hours = params.get("hours")?.as_u64()?;
///     Some(Arc::new(LastAccessOlderThan::new(hours)))
/// });
///
/// registry.register("IsFile", |_| Some(Arc::new(IsFile)));
///
/// // Build from JSON config
/// let rule = registry.build(&config)?;
/// ```
pub struct SpecRegistry<Ctx: Send + Sync + 'static> {
    factories: HashMap<String, SpecFactory<Ctx>>,
}

impl<Ctx: Send + Sync + 'static> Default for SpecRegistry<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx: Send + Sync + 'static> SpecRegistry<Ctx> {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a specification factory.
    ///
    /// The factory receives a HashMap of parameters and returns an optional
    /// Arc-wrapped specification. Return `None` if parameters are invalid.
    pub fn register<F>(&mut self, name: impl Into<String>, factory: F)
    where
        F: Fn(&HashMap<String, Value>) -> Option<BoxedSpec<Ctx>> + Send + Sync + 'static,
    {
        self.factories.insert(name.into(), Arc::new(factory));
    }

    /// Check if a specification is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.factories.contains_key(name)
    }

    /// List all registered specification names.
    pub fn list(&self) -> Vec<&str> {
        self.factories.keys().map(|s| s.as_str()).collect()
    }

    /// Create a specification by name with parameters.
    pub fn create(&self, name: &str, params: &HashMap<String, Value>) -> Option<BoxedSpec<Ctx>> {
        self.factories.get(name).and_then(|factory| factory(params))
    }

    /// Build a specification tree from a RuleConfig.
    ///
    /// Returns `None` if any referenced spec is not registered or
    /// if parameters are invalid.
    pub fn build(&self, config: &RuleConfig) -> Option<BoxedSpec<Ctx>> {
        match config {
            RuleConfig::Spec { name, params } => {
                self.create(name, params)
            }
            RuleConfig::And { rules } => {
                let specs: Option<Vec<BoxedSpec<Ctx>>> = rules
                    .iter()
                    .map(|r| self.build(r))
                    .collect();
                let specs = specs?;
                if specs.is_empty() {
                    return None;
                }
                Some(Arc::new(AllOf::new(specs)))
            }
            RuleConfig::Or { rules } => {
                let specs: Option<Vec<BoxedSpec<Ctx>>> = rules
                    .iter()
                    .map(|r| self.build(r))
                    .collect();
                let specs = specs?;
                if specs.is_empty() {
                    return None;
                }
                Some(Arc::new(AnyOf::new(specs)))
            }
            RuleConfig::Not { rule } => {
                let inner = self.build(rule)?;
                Some(Arc::new(Not(inner)))
            }
        }
    }

    /// Validate a RuleConfig without building it.
    ///
    /// Returns a list of errors if the config references unknown specs
    /// or has invalid structure.
    pub fn validate(&self, config: &RuleConfig) -> Vec<String> {
        let mut errors = Vec::new();
        self.validate_recursive(config, &mut errors);
        errors
    }

    fn validate_recursive(&self, config: &RuleConfig, errors: &mut Vec<String>) {
        match config {
            RuleConfig::Spec { name, params } => {
                if !self.contains(name) {
                    errors.push(format!("Unknown specification: {}", name));
                } else if self.create(name, params).is_none() {
                    errors.push(format!(
                        "Invalid parameters for specification '{}': {:?}",
                        name, params
                    ));
                }
            }
            RuleConfig::And { rules } | RuleConfig::Or { rules } => {
                if rules.is_empty() {
                    errors.push("Empty AND/OR rule list".to_string());
                }
                for rule in rules {
                    self.validate_recursive(rule, errors);
                }
            }
            RuleConfig::Not { rule } => {
                self.validate_recursive(rule, errors);
            }
        }
    }
}

// =============================================================================
// Pre-built registries for common contexts
// =============================================================================

use crate::context::FileContext;
use crate::rules::*;

/// Create a pre-configured registry for FileContext (Horus cleanup rules).
pub fn file_context_registry() -> SpecRegistry<FileContext> {
    let mut registry = SpecRegistry::new();

    registry.register("LastAccessOlderThan", |params| {
        let hours = params.get("hours")?.as_u64()?;
        Some(Arc::new(LastAccessOlderThan::new(hours)))
    });

    registry.register("CreatedOlderThan", |params| {
        let hours = params.get("hours")?.as_u64()?;
        Some(Arc::new(CreatedOlderThan::new(hours)))
    });

    registry.register("IsFile", |_| Some(Arc::new(IsFile)));

    registry.register("IsDirectory", |_| Some(Arc::new(IsDirectory)));

    registry.register("SizeLargerThan", |params| {
        let bytes = params.get("bytes")?.as_u64()?;
        Some(Arc::new(SizeLargerThan::new(bytes)))
    });

    registry
}

use crate::context::ExecutionContext;

/// Create a pre-configured registry for ExecutionContext (Minos judge rules).
pub fn execution_context_registry() -> SpecRegistry<ExecutionContext> {
    let mut registry = SpecRegistry::new();

    registry.register("WithinTimeLimit", |_| Some(Arc::new(WithinTimeLimit)));

    registry.register("WithinMemoryLimit", |_| Some(Arc::new(WithinMemoryLimit)));

    registry.register("ExitCodeZero", |_| Some(Arc::new(ExitCodeZero)));

    registry.register("OutputMatches", |_| Some(Arc::new(OutputMatches)));

    registry
}

#[cfg(feature = "auth")]
use crate::auth_rules::*;
#[cfg(feature = "auth")]
use crate::context::AuthContext;

/// Create a pre-configured registry for AuthContext (Vanguard authorization).
#[cfg(feature = "auth")]
pub fn auth_context_registry() -> SpecRegistry<AuthContext> {
    let mut registry = SpecRegistry::new();

    // User-level rules
    registry.register("IsValidUser", |_| Some(Arc::new(IsValidUser)));
    registry.register("IsAdmin", |_| Some(Arc::new(IsAdmin)));
    registry.register("IsOrganizer", |_| Some(Arc::new(IsOrganizer)));

    // Contest-scoped rules
    registry.register("IsParticipant", |_| Some(Arc::new(IsParticipant)));
    registry.register("IsCollaborator", |_| Some(Arc::new(IsCollaborator)));
    registry.register("IsContestOwner", |_| Some(Arc::new(IsContestOwner)));
    registry.register("CanAddProblems", |_| Some(Arc::new(CanAddProblems)));

    // Problem-scoped rules
    registry.register("IsProblemOwner", |_| Some(Arc::new(IsProblemOwner)));
    registry.register("CanAccessProblemBinaries", |_| {
        Some(Arc::new(CanAccessProblemBinaries))
    });

    // Submission-scoped rules
    registry.register("IsSubmissionOwner", |_| Some(Arc::new(IsSubmissionOwner)));

    // Rate limiting rules (with parameters)
    registry.register("NotRateLimited", |params| {
        let action = params.get("action")?.as_str()?;
        let limit = params.get("limit")?.as_u64()?;
        let window_secs = params.get("window_secs")?.as_u64()?;
        Some(Arc::new(NotRateLimited::new(action, limit, window_secs)))
    });

    // Pre-configured rate limit shortcuts
    registry.register("NotRateLimited:submission", |_| {
        Some(Arc::new(NotRateLimited::submission()))
    });

    registry.register("NotRateLimited:api", |_| {
        Some(Arc::new(NotRateLimited::api_authenticated()))
    });

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_registry_basic() {
        let registry = file_context_registry();
        
        assert!(registry.contains("IsFile"));
        assert!(registry.contains("LastAccessOlderThan"));
        assert!(!registry.contains("NonExistent"));
    }

    #[test]
    fn test_file_registry_create() {
        let registry = file_context_registry();
        
        // Parameterless spec
        let spec = registry.create("IsFile", &HashMap::new());
        assert!(spec.is_some());
        
        // Spec with params
        let mut params = HashMap::new();
        params.insert("hours".to_string(), serde_json::json!(6));
        let spec = registry.create("LastAccessOlderThan", &params);
        assert!(spec.is_some());
        
        // Invalid params
        let mut bad_params = HashMap::new();
        bad_params.insert("wrong".to_string(), serde_json::json!("key"));
        let spec = registry.create("LastAccessOlderThan", &bad_params);
        assert!(spec.is_none());
    }

    #[test]
    fn test_build_from_config() {
        let registry = file_context_registry();
        
        let mut params = HashMap::new();
        params.insert("hours".to_string(), serde_json::json!(6));
        
        let config = RuleConfig::And {
            rules: vec![
                RuleConfig::Spec {
                    name: "LastAccessOlderThan".to_string(),
                    params,
                },
                RuleConfig::Spec {
                    name: "IsFile".to_string(),
                    params: HashMap::new(),
                },
            ],
        };
        
        let spec = registry.build(&config);
        assert!(spec.is_some());
    }

    #[test]
    fn test_validate_config() {
        let registry = file_context_registry();
        
        // Valid config
        let config = RuleConfig::And {
            rules: vec![
                RuleConfig::Spec {
                    name: "IsFile".to_string(),
                    params: HashMap::new(),
                },
            ],
        };
        let errors = registry.validate(&config);
        assert!(errors.is_empty());
        
        // Invalid config - unknown spec
        let bad_config = RuleConfig::Spec {
            name: "UnknownSpec".to_string(),
            params: HashMap::new(),
        };
        let errors = registry.validate(&bad_config);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("Unknown specification"));
    }

    #[test]
    fn test_list_specs() {
        let registry = file_context_registry();
        let specs = registry.list();
        
        assert!(specs.contains(&"IsFile"));
        assert!(specs.contains(&"IsDirectory"));
        assert!(specs.contains(&"LastAccessOlderThan"));
    }
}
