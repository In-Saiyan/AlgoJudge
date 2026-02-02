//! Core Specification trait and combinators.

use async_trait::async_trait;
use std::marker::PhantomData;
use std::sync::Arc;

/// Core specification trait for composable business rules.
/// 
/// The Specification Pattern allows you to compose complex business rules
/// from simple, reusable predicates.
#[async_trait]
pub trait Specification<Ctx>: Send + Sync {
    /// Check if the specification is satisfied by the given context.
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool;

    /// Combine this specification with another using AND logic.
    fn and<S: Specification<Ctx>>(self, other: S) -> And<Self, S>
    where
        Self: Sized,
    {
        And(self, other)
    }

    /// Combine this specification with another using OR logic.
    fn or<S: Specification<Ctx>>(self, other: S) -> Or<Self, S>
    where
        Self: Sized,
    {
        Or(self, other)
    }

    /// Negate this specification.
    fn not(self) -> Not<Self>
    where
        Self: Sized,
    {
        Not(self)
    }
}

/// AND combinator for specifications.
#[derive(Clone)]
pub struct And<A, B>(pub A, pub B);

#[async_trait]
impl<Ctx, A, B> Specification<Ctx> for And<A, B>
where
    Ctx: Send + Sync,
    A: Specification<Ctx>,
    B: Specification<Ctx>,
{
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        self.0.is_satisfied_by(ctx).await && self.1.is_satisfied_by(ctx).await
    }
}

/// OR combinator for specifications.
#[derive(Clone)]
pub struct Or<A, B>(pub A, pub B);

#[async_trait]
impl<Ctx, A, B> Specification<Ctx> for Or<A, B>
where
    Ctx: Send + Sync,
    A: Specification<Ctx>,
    B: Specification<Ctx>,
{
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        self.0.is_satisfied_by(ctx).await || self.1.is_satisfied_by(ctx).await
    }
}

/// NOT combinator for specifications.
#[derive(Clone)]
pub struct Not<A>(pub A);

#[async_trait]
impl<Ctx, A> Specification<Ctx> for Not<A>
where
    Ctx: Send + Sync,
    A: Specification<Ctx>,
{
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        !self.0.is_satisfied_by(ctx).await
    }
}

/// A specification that always returns true.
#[derive(Clone, Copy)]
pub struct AlwaysTrue;

#[async_trait]
impl<Ctx: Send + Sync> Specification<Ctx> for AlwaysTrue {
    async fn is_satisfied_by(&self, _ctx: &Ctx) -> bool {
        true
    }
}

/// A specification that always returns false.
#[derive(Clone, Copy)]
pub struct AlwaysFalse;

#[async_trait]
impl<Ctx: Send + Sync> Specification<Ctx> for AlwaysFalse {
    async fn is_satisfied_by(&self, _ctx: &Ctx) -> bool {
        false
    }
}

/// A boxed specification for dynamic dispatch.
pub type BoxedSpec<Ctx> = Arc<dyn Specification<Ctx>>;

#[async_trait]
impl<Ctx: Send + Sync> Specification<Ctx> for BoxedSpec<Ctx> {
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        self.as_ref().is_satisfied_by(ctx).await
    }
}

/// All specifications in the collection must be satisfied.
pub struct AllOf<Ctx> {
    specs: Vec<BoxedSpec<Ctx>>,
    _phantom: PhantomData<Ctx>,
}

impl<Ctx> AllOf<Ctx> {
    pub fn new(specs: Vec<BoxedSpec<Ctx>>) -> Self {
        AllOf {
            specs,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<Ctx: Send + Sync> Specification<Ctx> for AllOf<Ctx> {
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        for spec in &self.specs {
            if !spec.is_satisfied_by(ctx).await {
                return false;
            }
        }
        true
    }
}

/// Any specification in the collection must be satisfied.
pub struct AnyOf<Ctx> {
    specs: Vec<BoxedSpec<Ctx>>,
    _phantom: PhantomData<Ctx>,
}

impl<Ctx> AnyOf<Ctx> {
    pub fn new(specs: Vec<BoxedSpec<Ctx>>) -> Self {
        AnyOf {
            specs,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl<Ctx: Send + Sync> Specification<Ctx> for AnyOf<Ctx> {
    async fn is_satisfied_by(&self, ctx: &Ctx) -> bool {
        for spec in &self.specs {
            if spec.is_satisfied_by(ctx).await {
                return true;
            }
        }
        false
    }
}
