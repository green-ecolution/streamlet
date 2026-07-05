use std::time::Duration;

use server::config::Settings;
use server::startup::Application;
use wiremock::MockServer;

pub struct TestApp {
    pub address: String,
    pub valhalla: MockServer,
    pub client: reqwest::Client,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let valhalla = MockServer::start().await;
        let settings = Settings {
            addr: "127.0.0.1:0".into(),
            valhalla_url: valhalla.uri(),
            engine_timeout: Duration::from_secs(2),
            solver_time_limit: Duration::from_millis(200),
        };
        let app = Application::build(&settings)
            .await
            .expect("failed to build app");
        let address = format!("http://{}", app.addr);
        tokio::spawn(app.run());
        Self {
            address,
            valhalla,
            client: reqwest::Client::new(),
        }
    }

    pub async fn post_json(&self, path: &str, body: &serde_json::Value) -> reqwest::Response {
        self.client
            .post(format!("{}{path}", self.address))
            .json(body)
            .send()
            .await
            .unwrap()
    }

    pub async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.address))
            .send()
            .await
            .unwrap()
    }
}

/// A minimal valid solve request: 1 vehicle, 1 depot, 2 customers, 1 refill (5 nodes).
pub fn solve_request() -> serde_json::Value {
    serde_json::json!({
        "problem": {
            "vehicles": [{
                "id": 1,
                "start": {"lat": 54.78, "lon": 9.43},
                "tank": {"capacity": 100.0, "level": 100.0},
                "kind": {"Car": {"width": 2.0, "height": 2.0}},
                "shift": {"start": 0.0, "end": 28800.0},
                "max_trips": null
            }],
            "depots": [{"id": 1, "location": {"lat": 54.78, "lon": 9.43}}],
            "customers": [
                {"id": 1, "location": {"lat": 54.79, "lon": 9.44},
                 "demand": 40.0, "service_time": 300.0, "time_window": null},
                {"id": 2, "location": {"lat": 54.80, "lon": 9.45},
                 "demand": 40.0, "service_time": 300.0, "time_window": null}
            ],
            "refill_stations": [{"id": 1, "location": {"lat": 54.785, "lon": 9.435},
                                 "refill_duration": 600.0}]
        },
        "options": {"geometry": "polyline"}
    })
}

/// Mounts a Valhalla matrix mock for `n` locations plus a route mock.
///
/// Cell (i, j) is 0 when i == j (the diagonal) and 120s / 1.5km otherwise, so
/// the resulting matrix is feasible for `solve_request()`'s problem (shift of
/// 8h, tank capacity comfortably covering both customers' demand).
pub async fn mock_valhalla(server: &MockServer, n: usize) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let rows: Vec<Vec<serde_json::Value>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        serde_json::json!({"time": 0, "distance": 0.0})
                    } else {
                        serde_json::json!({"time": 120, "distance": 1.5})
                    }
                })
                .collect()
        })
        .collect();
    Mock::given(method("POST"))
        .and(path("/sources_to_targets"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"sources_to_targets": rows})),
        )
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/route"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "trip": {"legs": [{"shape": "mock_shape"}]}
        })))
        .mount(server)
        .await;
}
