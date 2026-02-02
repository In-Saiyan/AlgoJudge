//! JSON configuration for dynamic rule building.
//!
//! Rules can be serialized to and deserialized from JSON, allowing
//! admin configuration through dashboards.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON representation of a rule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleConfig {
    /// A single specification by name
    Spec {
        name: String,
        #[serde(default)]
        params: HashMap<String, serde_json::Value>,
    },
    /// AND combination of rules
    And { rules: Vec<RuleConfig> },
    /// OR combination of rules
    Or { rules: Vec<RuleConfig> },
    /// Negation of a rule
    Not { rule: Box<RuleConfig> },
}

impl RuleConfig {
    /// Create a new spec rule
    pub fn spec(name: impl Into<String>) -> Self {
        RuleConfig::Spec {
            name: name.into(),
            params: HashMap::new(),
        }
    }

    /// Create a new spec rule with parameters
    pub fn spec_with_params(
        name: impl Into<String>,
        params: HashMap<String, serde_json::Value>,
    ) -> Self {
        RuleConfig::Spec {
            name: name.into(),
            params,
        }
    }

    /// Create an AND combination
    pub fn and(rules: Vec<RuleConfig>) -> Self {
        RuleConfig::And { rules }
    }

    /// Create an OR combination
    pub fn or(rules: Vec<RuleConfig>) -> Self {
        RuleConfig::Or { rules }
    }

    /// Create a NOT wrapper
    pub fn not(rule: RuleConfig) -> Self {
        RuleConfig::Not {
            rule: Box::new(rule),
        }
    }
}

/// A named rule configuration for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedRuleConfig {
    pub name: String,
    pub description: Option<String>,
    pub service: String, // "vanguard", "minos", "horus"
    pub rule: RuleConfig,
    pub version: String,
    pub enabled: bool,
}

/// Cleanup policy configuration for Horus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPolicy {
    pub name: String,
    pub description: Option<String>,
    pub target_path: String,
    pub rule: RuleConfig,
    pub action: CleanupAction,
    pub enabled: bool,
}

/// Action to take when a cleanup rule matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupAction {
    Delete,
    Archive,
    Move,
    Log,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_config_serialization() {
        let rule = RuleConfig::and(vec![
            RuleConfig::spec("IsValidUser"),
            RuleConfig::or(vec![
                RuleConfig::and(vec![
                    RuleConfig::not(RuleConfig::spec("IsRateLimited")),
                    RuleConfig::spec("IsParticipant"),
                ]),
                RuleConfig::spec("IsAdmin"),
            ]),
        ]);

        let json = serde_json::to_string_pretty(&rule).unwrap();
        println!("{}", json);

        let parsed: RuleConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, RuleConfig::And { .. }));
    }

    #[test]
    fn test_cleanup_policy_serialization() {
        let policy = CleanupPolicy {
            name: "stale_testcases".to_string(),
            description: Some("Remove test cases not accessed in 6 hours".to_string()),
            target_path: "/mnt/data/testcases".to_string(),
            rule: RuleConfig::and(vec![
                RuleConfig::spec_with_params(
                    "LastAccessOlderThan",
                    [("hours".to_string(), serde_json::json!(6))]
                        .into_iter()
                        .collect(),
                ),
                RuleConfig::spec("IsFile"),
            ]),
            action: CleanupAction::Delete,
            enabled: true,
        };

        let json = serde_json::to_string_pretty(&policy).unwrap();
        println!("{}", json);

        let parsed: CleanupPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "stale_testcases");
    }
}
