use crate::helpers::TestApp;

#[tokio::test]
async fn health_returns_ok() {
    let app = TestApp::spawn().await;
    let response = app.get("/health").await;
    assert_eq!(response.status(), 200);
}
