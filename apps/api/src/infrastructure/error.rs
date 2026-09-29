use crate::error::ApiError;
impl From<diesel::result::Error> for ApiError {
    fn from(e: diesel::result::Error) -> Self {
        match e {
            diesel::result::Error::NotFound => Self(404, "not_found", "This item is unavailable."),
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            ) => Self(
                409,
                "conflict",
                "This request conflicts with an existing account or record.",
            ),
            _ => Self::internal(e),
        }
    }
}
