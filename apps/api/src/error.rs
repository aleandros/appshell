use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}
#[derive(Debug)]
pub struct ApiError(pub u16, pub &'static str, pub &'static str);
pub type Result<T> = std::result::Result<T, ApiError>;
impl ApiError {
    pub fn bad(message: &'static str) -> Self {
        Self(400, "invalid_request", message)
    }
    pub fn unauthorized() -> Self {
        Self(401, "unauthenticated", "Please sign in to continue.")
    }
    pub fn forbidden() -> Self {
        Self(403, "forbidden", "You don't have permission to do that.")
    }
    pub fn internal(e: impl std::fmt::Display) -> Self {
        tracing::error!(error = %e, "request failed");
        Self(
            500,
            "internal_error",
            "Something went wrong. Please try again.",
        )
    }
}

impl From<appshell_domain::RuleError> for ApiError {
    fn from(error: appshell_domain::RuleError) -> Self {
        use appshell_domain::RuleError;
        match error {
            RuleError::Invalid(message) => Self::bad(message),
            RuleError::Forbidden => Self::forbidden(),
            RuleError::EmailUnverified => Self(
                403,
                "email_unverified",
                "Verify your email before continuing.",
            ),
            RuleError::RateLimited => Self(
                429,
                "rate_limited",
                "Too many attempts. Please try again in 15 minutes.",
            ),
        }
    }
}
