//! Specification Pattern implementation for composable business rules.
//!
//! This crate provides a flexible rule engine using the Specification Pattern,
//! allowing you to compose complex business rules from simple, reusable predicates.
//!
//! # Example
//!
//! ```ignore
//! use olympus_rules::prelude::*;
//!
//! // Compose rules with operators
//! let can_submit = Spec(IsValidUser) & ((!Spec(IsRateLimited) & Spec(IsParticipant)) | Spec(IsAdmin));
//!
//! // Evaluate
//! if can_submit.is_satisfied_by(&context).await {
//!     // Allow submission
//! }
//! ```
//!
//! # Features
//!
//! - `auth` - Enable authorization rules that require database/Redis access

pub mod config;
pub mod context;
pub mod operators;
pub mod registry;
pub mod rules;
pub mod specification;

#[cfg(feature = "auth")]
pub mod auth_rules;

/// Prelude module - import everything you need with `use olympus_rules::prelude::*`
pub mod prelude {
    pub use crate::config::{CleanupAction, CleanupPolicy, NamedRuleConfig, RuleConfig};
    pub use crate::context::{EvalContext, ExecutionContext, FileContext};
    pub use crate::operators::Spec;
    pub use crate::registry::{execution_context_registry, file_context_registry, SpecRegistry};
    pub use crate::rules::*;
    pub use crate::specification::{
        AllOf, AlwaysFalse, AlwaysTrue, And, AnyOf, BoxedSpec, Not, Or, Specification,
    };

    #[cfg(feature = "auth")]
    pub use crate::auth_rules::*;
    #[cfg(feature = "auth")]
    pub use crate::context::AuthContext;
    #[cfg(feature = "auth")]
    pub use crate::registry::auth_context_registry;
}
