use crate::error::{ApiError, ErrorBody, Result};
use axum::{
    Json,
    response::{IntoResponse, Response},
};
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            axum::http::StatusCode::from_u16(self.0)
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            Json(ErrorBody {
                code: self.1.into(),
                message: self.2.into(),
            }),
        )
            .into_response()
    }
}
pub struct ApiJson<T>(pub T);
impl<S, T> axum::extract::FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = ApiError;
    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|_| ApiError::bad("Please check the submitted fields."))?;
        Ok(Self(value))
    }
}
