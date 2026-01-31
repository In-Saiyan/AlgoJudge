//! Database repositories
//!
//! Repositories handle all direct database interactions.

pub mod contest_repo;
pub mod problem_repo;
pub mod submission_repo;
pub mod user_repo;

pub use contest_repo::ContestRepository;
pub use problem_repo::ProblemRepository;
pub use submission_repo::SubmissionRepository;
pub use user_repo::UserRepository;
