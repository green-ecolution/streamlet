use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use streamlet_core::router::RouterError;

use crate::service::ServiceError;

#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        // Log the specifics server-side; answer with generic, leak-free messages.
        tracing::error!(error = %self, "solve request failed");
        let (status, message) = match &self {
            ServiceError::Router(RouterError::Timeout) => {
                (StatusCode::GATEWAY_TIMEOUT, "routing engine timed out")
            }
            ServiceError::Router(_) => (StatusCode::BAD_GATEWAY, "routing engine unavailable"),
            ServiceError::Solver(_) | ServiceError::SolverPanic => {
                (StatusCode::INTERNAL_SERVER_ERROR, "optimization failed")
            }
        };
        (
            status,
            Json(ErrorBody {
                error: message.into(),
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use streamlet_core::router::RouterError;

    use crate::service::ServiceError;

    fn status_of(err: ServiceError) -> axum::http::StatusCode {
        err.into_response().status()
    }

    #[tokio::test]
    async fn router_errors_map_to_gateway_statuses() {
        assert_eq!(
            status_of(ServiceError::Router(RouterError::Timeout)),
            axum::http::StatusCode::GATEWAY_TIMEOUT
        );
        assert_eq!(
            status_of(ServiceError::Router(RouterError::Unreachable("x".into()))),
            axum::http::StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            status_of(ServiceError::Router(RouterError::Rejected("x".into()))),
            axum::http::StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            status_of(ServiceError::Router(RouterError::InvalidResponse(
                "x".into()
            ))),
            axum::http::StatusCode::BAD_GATEWAY
        );
    }

    #[tokio::test]
    async fn internal_details_never_leak() {
        let err = ServiceError::Router(RouterError::Unreachable(
            "http://secret-host:8002 refused".into(),
        ));
        let response = err.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(!text.contains("secret-host"), "leaked: {text}");
    }
}
