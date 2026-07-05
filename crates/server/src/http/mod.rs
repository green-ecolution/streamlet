use std::sync::Arc;

use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::service::SolveService;

pub mod v1;

pub struct AppState {
    pub solve_service: Arc<SolveService>,
}

#[derive(OpenApi)]
#[openapi(info(
    title = "Streamlet",
    description = "Route optimization service (VRP with time windows and refill stations)"
))]
struct ApiDoc;

pub fn router(state: Arc<AppState>) -> axum::Router {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(v1::solve::solve))
        .routes(routes!(v1::health::health))
        .with_state(state)
        .split_for_parts();
    router.route(
        "/api-docs/openapi.json",
        axum::routing::get(move || {
            let api = api.clone();
            async move { axum::Json(api) }
        }),
    )
}
