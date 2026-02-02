//! Operator overloading for specification combinators.
//!
//! This module provides implementations of `BitAnd` (&), `BitOr` (|), and `Not` (!)\n//! for specifications, allowing intuitive syntax like:
//!
//! ```ignore
//! let rule = Spec(IsValidUser) & Spec(IsParticipant);
//! ```

use crate::specification::{And, Not, Or, Specification};
use std::ops::{BitAnd, BitOr, Not as StdNot};

/// Wrapper struct to enable operator overloading on specifications.
/// 
/// Use this wrapper to compose specifications with operators:
/// ```ignore
/// let rule = Spec(IsValidUser) & Spec(IsParticipant);
/// ```
#[derive(Clone)]
pub struct Spec<S>(pub S);

impl<A, B> BitAnd<Spec<B>> for Spec<A> {
    type Output = Spec<And<A, B>>;

    fn bitand(self, rhs: Spec<B>) -> Self::Output {
        Spec(And(self.0, rhs.0))
    }
}

impl<A, B> BitOr<Spec<B>> for Spec<A> {
    type Output = Spec<Or<A, B>>;

    fn bitor(self, rhs: Spec<B>) -> Self::Output {
        Spec(Or(self.0, rhs.0))
    }
}

impl<A> StdNot for Spec<A> {
    type Output = Spec<Not<A>>;

    fn not(self) -> Self::Output {
        Spec(Not(self.0))
    }
}

use async_trait::async_trait;

#[async_trait]
impl<Ctx, S> Specification<Ctx> for Spec<S>
where
    Ctx: Send + Sync,
    S: Specification<Ctx>,
{
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        self.0.is_satisfied_by(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specification::{AlwaysFalse, AlwaysTrue};

    #[tokio::test]
    async fn test_and_operator() {
        let rule = Spec(AlwaysTrue) & Spec(AlwaysTrue);
        assert!(rule.is_satisfied_by(&()).await);

        let rule = Spec(AlwaysTrue) & Spec(AlwaysFalse);
        assert!(!rule.is_satisfied_by(&()).await);
    }

    #[tokio::test]
    async fn test_or_operator() {
        let rule = Spec(AlwaysFalse) | Spec(AlwaysTrue);
        assert!(rule.is_satisfied_by(&()).await);

        let rule = Spec(AlwaysFalse) | Spec(AlwaysFalse);
        assert!(!rule.is_satisfied_by(&()).await);
    }

    #[tokio::test]
    async fn test_not_operator() {
        let rule = !Spec(AlwaysTrue);
        assert!(!rule.is_satisfied_by(&()).await);

        let rule = !Spec(AlwaysFalse);
        assert!(rule.is_satisfied_by(&()).await);
    }

    #[tokio::test]
    async fn test_complex_expression() {
        // (true & (!false)) | false = true
        let rule = (Spec(AlwaysTrue) & !Spec(AlwaysFalse)) | Spec(AlwaysFalse);
        assert!(rule.is_satisfied_by(&()).await);
    }
}
