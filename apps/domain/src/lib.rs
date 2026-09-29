pub mod admin;
// Pure business rules. Inputs include all facts; this crate never performs I/O.
pub mod billing;
pub mod identity;
pub mod organizations;

#[derive(Debug, PartialEq, Eq)]
pub enum RuleError {
    Invalid(&'static str),
    Forbidden,
    EmailUnverified,
    RateLimited,
}
pub type Result<T> = std::result::Result<T, RuleError>;
