use crate::helpers::{TestApp, mock_valhalla, solve_request};

#[tokio::test]
async fn solve_returns_routes_with_geometry() {
    let app = TestApp::spawn().await;
    mock_valhalla(&app.valhalla, 5).await;
    let response = app.post_json("/v1/solve", &solve_request()).await;
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["routes"].as_array().unwrap().len(), 1);
    assert_eq!(body["unserved"].as_array().unwrap().len(), 0);
    let stops = body["routes"][0]["stops"].as_array().unwrap();
    assert!(
        stops.len() >= 4,
        "start + 2 customers + depot, got {stops:?}"
    );
    assert!(body["routes"][0]["geometry"].is_object());
}

#[tokio::test]
async fn invalid_problem_is_rejected() {
    let app = TestApp::spawn().await;
    let mut request = solve_request();
    request["problem"]["vehicles"] = serde_json::json!([]); // Problem requires >= 1 vehicle
    let response = app.post_json("/v1/solve", &request).await;
    assert_eq!(response.status(), 422);
}

#[tokio::test]
async fn overfilled_tank_is_rejected() {
    let app = TestApp::spawn().await;
    let mut request = solve_request();
    request["problem"]["vehicles"][0]["tank"] =
        serde_json::json!({"capacity": 100.0, "level": 200.0});
    let response = app.post_json("/v1/solve", &request).await;
    assert_eq!(response.status(), 422);
}

#[tokio::test]
async fn unreachable_engine_maps_to_502() {
    let app = TestApp::spawn().await;
    // No mocks mounted: wiremock answers 404, which the client maps to Rejected -> 502.
    let response = app.post_json("/v1/solve", &solve_request()).await;
    assert_eq!(response.status(), 502);
}
