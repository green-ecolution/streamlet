use serde::{Deserialize, Serialize};

use crate::domain::{Coordinate, Vehicle};
use crate::matrix::CostMatrix;

#[derive(Debug, thiserror::Error)]
pub enum RouterError {
    #[error("routing engine unreachable: {0}")]
    Unreachable(String),
    #[error("routing engine rejected the request: {0}")]
    Rejected(String),
    #[error("routing engine returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("routing engine timed out")]
    Timeout,
}

/// Route geometry as returned by the routing engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "format", content = "value", rename_all = "snake_case")]
pub enum RouteGeometry {
    /// Encoded polyline (Valhalla: precision 6). Multi-leg routes join leg
    /// shapes with `;`.
    Polyline(String),
    /// GeoJSON LineString geometry object.
    GeoJson(serde_json::Value),
}

/// Port to an external routing engine (Valhalla, OSRM, or an embedded engine).
///
/// `matrix` must return costs over `locations` in the given order — the solver
/// relies on index alignment with its node layout.
#[async_trait::async_trait]
pub trait Router: Send + Sync {
    /// Travel-time (seconds) / distance (meters) matrix over `locations`,
    /// respecting vehicle costing (dimensions, weight).
    async fn matrix(
        &self,
        vehicle: &Vehicle,
        locations: &[Coordinate],
    ) -> Result<CostMatrix, RouterError>;

    /// Geometry along the ordered `waypoints` for the given vehicle.
    async fn directions(
        &self,
        vehicle: &Vehicle,
        waypoints: &[Coordinate],
    ) -> Result<RouteGeometry, RouterError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Coordinate, Vehicle};

    struct FakeRouter;

    #[async_trait::async_trait]
    impl Router for FakeRouter {
        async fn matrix(
            &self,
            _vehicle: &Vehicle,
            locations: &[Coordinate],
        ) -> Result<CostMatrix, RouterError> {
            let n = locations.len();
            let zeros = vec![vec![0.0; n]; n];
            Ok(CostMatrix::new(zeros.clone(), zeros).expect("square"))
        }

        async fn directions(
            &self,
            _vehicle: &Vehicle,
            _waypoints: &[Coordinate],
        ) -> Result<RouteGeometry, RouterError> {
            Ok(RouteGeometry::Polyline("_p~iF~ps|U".into()))
        }
    }

    #[test]
    fn router_is_object_safe() {
        let _boxed: Box<dyn Router> = Box::new(FakeRouter);
    }

    #[tokio::test]
    async fn fake_router_roundtrip() {
        let router: Box<dyn Router> = Box::new(FakeRouter);
        let problem = crate::solver::route::tests::problem_two_customers();
        let vehicle = problem.vehicles()[0];
        let locations = [Coordinate::new(54.0, 9.0).unwrap()];
        let matrix = router.matrix(&vehicle, &locations).await.unwrap();
        assert_eq!(matrix.len(), 1);
        assert!(matches!(
            router.directions(&vehicle, &locations).await.unwrap(),
            RouteGeometry::Polyline(_)
        ));
    }
}
