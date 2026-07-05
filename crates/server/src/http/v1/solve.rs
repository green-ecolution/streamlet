use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::http::AppState;
use crate::http::v1::dto::{SolveRequest, SolveResponse};
use crate::service::ServiceError;

#[utoipa::path(
    post,
    path = "/v1/solve",
    request_body = SolveRequest,
    responses(
        (status = 200, description = "Optimized routes", body = SolveResponse),
        (status = 422, description = "Invalid problem (fails domain validation)"),
        (status = 502, description = "Routing engine unavailable"),
        (status = 504, description = "Routing engine timed out"),
    ),
    tag = "solve",
)]
pub async fn solve(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SolveRequest>,
) -> Result<Json<SolveResponse>, ServiceError> {
    let time_limit = request
        .options
        .time_limit_ms
        .map(std::time::Duration::from_millis);
    let result = state
        .solve_service
        .solve(request.problem, request.options.geometry.into(), time_limit)
        .await?;
    Ok(Json(result.into()))
}
