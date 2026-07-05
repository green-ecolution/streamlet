#[utoipa::path(get, path = "/health", responses((status = 200, description = "Service is up")), tag = "health")]
pub async fn health() -> &'static str {
    "ok"
}
