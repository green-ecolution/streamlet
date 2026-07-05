//! HTTP client for [Valhalla](https://valhalla.github.io/valhalla/)'s routing
//! engine, implementing the `streamlet_core` [`Router`] port.
//!
//! Talks to two Valhalla endpoints:
//! - `POST {base}/sources_to_targets` for the travel-time/distance matrix.
//!   Valhalla reports distance in kilometers; we convert to meters.
//! - `POST {base}/route` for turn-by-turn directions. Multi-leg routes are
//!   flattened by joining each leg's encoded polyline with `;`.
//!
//! Truck costing weight is converted from kilograms (our domain unit) to
//! metric tons (Valhalla's expected unit) by dividing by 1000.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use streamlet_core::domain::{Coordinate, Vehicle, VehicleKind};
use streamlet_core::matrix::CostMatrix;
use streamlet_core::router::{RouteGeometry, Router, RouterError};

pub struct ValhallaClient {
    base_url: String,
    http: reqwest::Client,
}

impl ValhallaClient {
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Result<Self, RouterError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| RouterError::Unreachable(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into(),
            http,
        })
    }

    async fn post(&self, endpoint: &str, body: Value) -> Result<Value, RouterError> {
        let url = format!("{}/{endpoint}", self.base_url.trim_end_matches('/'));
        let response = self.http.post(&url).json(&body).send().await.map_err(|e| {
            if e.is_timeout() {
                RouterError::Timeout
            } else {
                RouterError::Unreachable(e.to_string())
            }
        })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| RouterError::InvalidResponse(e.to_string()))?;
        if !status.is_success() {
            return Err(RouterError::Rejected(format!("{status}: {text}")));
        }
        serde_json::from_str(&text).map_err(|e| RouterError::InvalidResponse(e.to_string()))
    }

    /// Valhalla costing model name and options for `vehicle`.
    fn costing(vehicle: &Vehicle) -> (&'static str, Value) {
        match vehicle.kind {
            VehicleKind::Car { width, height } => (
                "auto",
                json!({
                    "auto": {"width": width.get(), "height": height.get()}
                }),
            ),
            VehicleKind::Truck {
                width,
                height,
                length,
                weight,
            } => (
                "truck",
                json!({
                    "truck": {
                        "width": width.get(),
                        "height": height.get(),
                        "length": length.get(),
                        // Valhalla expects metric tons; our domain uses kilograms.
                        "weight": weight.get() / 1000.0
                    }
                }),
            ),
        }
    }

    fn to_locations(coords: &[Coordinate]) -> Vec<Value> {
        coords
            .iter()
            .map(|c| json!({"lat": c.lat(), "lon": c.lon()}))
            .collect()
    }
}

#[derive(Deserialize)]
struct MatrixCell {
    #[serde(default)]
    time: Option<f64>,
    #[serde(default)]
    distance: Option<f64>,
}

#[async_trait::async_trait]
impl Router for ValhallaClient {
    /// Does not chunk requests: assumes problem-scale inputs (at most a few
    /// hundred locations), all sent to Valhalla in a single call.
    async fn matrix(
        &self,
        vehicle: &Vehicle,
        locations: &[Coordinate],
    ) -> Result<CostMatrix, RouterError> {
        if locations.is_empty() {
            // Nothing to ask the engine; CostMatrix::new(vec![], vec![]) cannot fail.
            return CostMatrix::new(vec![], vec![])
                .map_err(|e| RouterError::InvalidResponse(e.to_string()));
        }
        let (costing, options) = Self::costing(vehicle);
        let locations_json = Self::to_locations(locations);
        let body = json!({
            "sources": locations_json.clone(),
            "targets": locations_json,
            "costing": costing,
            "costing_options": options,
        });
        let value = self.post("sources_to_targets", body).await?;
        let rows: Vec<Vec<MatrixCell>> =
            serde_json::from_value(value.get("sources_to_targets").cloned().ok_or_else(|| {
                RouterError::InvalidResponse("missing sources_to_targets".into())
            })?)
            .map_err(|e| RouterError::InvalidResponse(e.to_string()))?;

        // The Router contract requires costs to be aligned by index with the
        // input locations; reject anything that doesn't come back square.
        let n = locations.len();
        if rows.len() != n || rows.iter().any(|row| row.len() != n) {
            return Err(RouterError::InvalidResponse(format!(
                "matrix size mismatch: expected {n}x{n}, got {}x?",
                rows.len()
            )));
        }

        // Unreachable pairs come back as null time/distance; reject rather
        // than silently punching holes the solver cannot handle.
        let cell = |v: Option<f64>| {
            v.ok_or_else(|| RouterError::InvalidResponse("unreachable location pair".into()))
        };
        let time: Vec<Vec<f64>> = rows
            .iter()
            .map(|row| row.iter().map(|c| cell(c.time)).collect())
            .collect::<Result<_, _>>()?;
        let distance: Vec<Vec<f64>> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| cell(c.distance).map(|km| km * 1000.0))
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        CostMatrix::new(time, distance).map_err(|e| RouterError::InvalidResponse(e.to_string()))
    }

    async fn directions(
        &self,
        vehicle: &Vehicle,
        waypoints: &[Coordinate],
    ) -> Result<RouteGeometry, RouterError> {
        let (costing, options) = Self::costing(vehicle);
        let locations: Vec<Value> = waypoints
            .iter()
            .map(|c| json!({"lat": c.lat(), "lon": c.lon(), "type": "break"}))
            .collect();
        let body = json!({
            "locations": locations,
            "costing": costing,
            "costing_options": options,
        });
        let value = self.post("route", body).await?;
        let legs = value
            .pointer("/trip/legs")
            .and_then(Value::as_array)
            .ok_or_else(|| RouterError::InvalidResponse("missing trip.legs".into()))?;
        let shapes: Vec<&str> = legs
            .iter()
            .map(|leg| {
                leg.get("shape")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RouterError::InvalidResponse("leg without shape".into()))
            })
            .collect::<Result<_, _>>()?;
        Ok(RouteGeometry::Polyline(shapes.join(";")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use streamlet_core::domain::*;
    use streamlet_core::matrix::NodeIndex;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn vehicle() -> Vehicle {
        Vehicle {
            id: VehicleId::new(1),
            start: Coordinate::new(54.78, 9.43).unwrap(),
            tank: Tank::full(Liters::new(100.0).unwrap()),
            kind: VehicleKind::Truck {
                width: Meters::new(2.5).unwrap(),
                height: Meters::new(3.2).unwrap(),
                length: Meters::new(7.0).unwrap(),
                weight: Kilograms::new(11_000.0).unwrap(),
            },
            shift: TimeWindow::new(Time::new(0.0).unwrap(), Time::new(28_800.0).unwrap()).unwrap(),
            max_trips: None,
        }
    }

    fn locations() -> Vec<Coordinate> {
        vec![
            Coordinate::new(54.78, 9.43).unwrap(),
            Coordinate::new(54.79, 9.44).unwrap(),
        ]
    }

    #[tokio::test]
    async fn matrix_converts_km_to_meters_and_uses_truck_costing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sources_to_targets"))
            .and(body_partial_json(serde_json::json!({
                "sources": [{"lat": 54.78, "lon": 9.43}, {"lat": 54.79, "lon": 9.44}],
                "costing": "truck",
                "costing_options": {"truck": {"weight": 11.0, "width": 2.5}}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sources_to_targets": [
                    [{"time": 0, "distance": 0.0}, {"time": 120, "distance": 1.5}],
                    [{"time": 130, "distance": 1.6}, {"time": 0, "distance": 0.0}]
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        let matrix = client.matrix(&vehicle(), &locations()).await.unwrap();
        assert_eq!(matrix.travel_time(NodeIndex(0), NodeIndex(1)), 120.0);
        assert_eq!(matrix.distance(NodeIndex(0), NodeIndex(1)), 1500.0);
    }

    #[tokio::test]
    async fn directions_joins_leg_polylines() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/route"))
            .and(body_partial_json(serde_json::json!({
                "locations": [
                    {"lat": 54.78, "lon": 9.43, "type": "break"},
                    {"lat": 54.79, "lon": 9.44, "type": "break"}
                ],
                "costing": "truck"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trip": {"legs": [{"shape": "abc"}, {"shape": "def"}]}
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        match client.directions(&vehicle(), &locations()).await.unwrap() {
            RouteGeometry::Polyline(shapes) => assert_eq!(shapes, "abc;def"),
            other => panic!("expected polyline, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn engine_4xx_maps_to_rejected() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sources_to_targets"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad costing"))
            .expect(1)
            .mount(&server)
            .await;
        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client.matrix(&vehicle(), &locations()).await.unwrap_err(),
            RouterError::Rejected(_)
        ));
    }

    #[tokio::test]
    async fn unreachable_pair_maps_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sources_to_targets"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sources_to_targets": [
                    [{"time": 0, "distance": 0.0}, {"time": null, "distance": null}],
                    [{"time": 130, "distance": 1.6}, {"time": 0, "distance": 0.0}]
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client.matrix(&vehicle(), &locations()).await.unwrap_err(),
            RouterError::InvalidResponse(_)
        ));
    }

    #[tokio::test]
    async fn wrong_matrix_dimension_maps_to_invalid_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sources_to_targets"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sources_to_targets": [
                    [{"time": 0, "distance": 0.0}]
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client.matrix(&vehicle(), &locations()).await.unwrap_err(),
            RouterError::InvalidResponse(_)
        ));
    }

    #[tokio::test]
    async fn timeout_maps_to_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sources_to_targets"))
            .respond_with(
                ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(200)),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client =
            ValhallaClient::new(server.uri(), std::time::Duration::from_millis(50)).unwrap();
        assert!(matches!(
            client.matrix(&vehicle(), &locations()).await.unwrap_err(),
            RouterError::Timeout
        ));
    }

    #[tokio::test]
    async fn unreachable_engine_maps_to_unreachable() {
        // No server listens on this port; the connection attempt itself fails.
        let client =
            ValhallaClient::new("http://127.0.0.1:1", std::time::Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client.matrix(&vehicle(), &locations()).await.unwrap_err(),
            RouterError::Unreachable(_)
        ));
    }

    #[tokio::test]
    async fn route_error_paths_map_to_invalid_response() {
        let missing_legs = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&missing_legs)
            .await;
        let client =
            ValhallaClient::new(missing_legs.uri(), std::time::Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client
                .directions(&vehicle(), &locations())
                .await
                .unwrap_err(),
            RouterError::InvalidResponse(_)
        ));

        let leg_without_shape = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/route"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "trip": {"legs": [{}]}
            })))
            .expect(1)
            .mount(&leg_without_shape)
            .await;
        let client =
            ValhallaClient::new(leg_without_shape.uri(), std::time::Duration::from_secs(1))
                .unwrap();
        assert!(matches!(
            client
                .directions(&vehicle(), &locations())
                .await
                .unwrap_err(),
            RouterError::InvalidResponse(_)
        ));
    }

    #[tokio::test]
    async fn empty_locations_yield_empty_matrix_without_http() {
        // No mocks mounted: any HTTP request would 404 and be rejected by
        // wiremock, so a passing result proves no call was made.
        let server = MockServer::start().await;
        let client = ValhallaClient::new(server.uri(), std::time::Duration::from_secs(1)).unwrap();
        let matrix = client.matrix(&vehicle(), &[]).await.unwrap();
        assert_eq!(matrix.len(), 0);
    }
}
