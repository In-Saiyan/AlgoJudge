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

pub mod specification;
pub mod operators;
pub mod context;
pub mod config;
pub mod rules;

#[cfg(feature = "auth")]
pub mod auth_rules;

/// Prelude module - import everything you need with `use olympus_rules::prelude::*`
pub mod prelude {
    pub use crate::specification::{
        Specification, And, Or, Not, AlwaysTrue, AlwaysFalse, BoxedSpec, AllOf, AnyOf,
    };
    pub use crate::operators::Spec;
    pub use crate::context::{EvalContext, FileContext, ExecutionContext};
    pub use crate::config::{RuleConfig, NamedRuleConfig, CleanupPolicy, CleanupAction};
    pub use crate::rules::*;
    
    #[cfg(feature = "auth")]
    pub use crate::context::AuthContext;
    #[cfg(feature = "auth")]
    pub use crate::auth_rules::*;
}
