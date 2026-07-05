use std::sync::Arc;
// Not imported by name: `streamlet_core::domain` also exports a `Duration`
// (seconds, non-negative), and this module's tests glob-import both
// (`use super::*;` + `use streamlet_core::domain::*;`) — a bare `use
// std::time::Duration;` here would make that glob ambiguous.
use std::time::Duration as StdDuration;

use streamlet_core::domain::{Coordinate, Problem, Solution, Stop};
use streamlet_core::router::{RouteGeometry, Router, RouterError};
use streamlet_core::solver::search::SearchLimits;
use streamlet_core::solver::{SolveOptions, SolverError, solve};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryFormat {
    None,
    Polyline,
    GeoJson,
}

/// One solved route's optional geometry, index-aligned with `solution.routes`.
#[derive(Debug)]
pub struct SolvedRoute {
    pub geometry: Option<RouteGeometry>,
}

#[derive(Debug)]
pub struct SolveResult {
    pub solution: Solution,
    pub routes: Vec<SolvedRoute>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(transparent)]
    Router(#[from] RouterError),
    #[error(transparent)]
    Solver(#[from] SolverError),
    #[error("solver task failed")]
    SolverPanic,
}

pub struct SolveService {
    router: Arc<dyn Router>,
    solver_time_limit: StdDuration,
}

impl SolveService {
    pub fn new(router: Arc<dyn Router>, solver_time_limit: StdDuration) -> Self {
        Self {
            router,
            solver_time_limit,
        }
    }

    /// All node coordinates in matrix order — MUST match `Instance::new` (core):
    /// vehicles, depots, customers, refill stations, in `Problem` field order.
    fn node_coordinates(problem: &Problem) -> Vec<Coordinate> {
        problem
            .vehicles()
            .iter()
            .map(|v| v.start)
            .chain(problem.depots().iter().map(|d| d.location))
            .chain(problem.customers().iter().map(|c| c.location))
            .chain(problem.refill_stations().iter().map(|r| r.location))
            .collect()
    }

    fn stop_coordinate(problem: &Problem, stop: &Stop) -> Coordinate {
        match stop {
            Stop::VehicleStart(id) => {
                problem
                    .vehicles()
                    .iter()
                    .find(|v| v.id == *id)
                    .expect("solver returns known ids")
                    .start
            }
            Stop::Depot(id) => {
                problem
                    .depots()
                    .iter()
                    .find(|d| d.id == *id)
                    .expect("solver returns known ids")
                    .location
            }
            Stop::Customer(id) => {
                problem
                    .customers()
                    .iter()
                    .find(|c| c.id == *id)
                    .expect("solver returns known ids")
                    .location
            }
            Stop::Refill(id) => {
                problem
                    .refill_stations()
                    .iter()
                    .find(|r| r.id == *id)
                    .expect("solver returns known ids")
                    .location
            }
        }
    }

    #[tracing::instrument(skip_all, fields(customers = problem.customers().len()))]
    pub async fn solve(
        &self,
        problem: Problem,
        geometry: GeometryFormat,
        time_limit: Option<StdDuration>,
    ) -> Result<SolveResult, ServiceError> {
        // Client may lower the budget, never raise it above the configured max.
        let time_limit = time_limit
            .map(|t| t.min(self.solver_time_limit))
            .unwrap_or(self.solver_time_limit);

        // Costing is per-vehicle, but one matrix serves all vehicles: use the
        // first vehicle's costing (heterogeneous fleets share the road graph;
        // keeps one matrix call per solve).
        let matrix_vehicle = problem.vehicles()[0];
        let coords = Self::node_coordinates(&problem);
        let matrix = self.router.matrix(&matrix_vehicle, &coords).await?;

        let options = SolveOptions {
            limits: SearchLimits {
                time_limit,
                ..SearchLimits::default()
            },
        };
        let solution = {
            let problem = problem.clone();
            tokio::task::spawn_blocking(move || solve(&problem, &matrix, &options))
                .await
                .map_err(|_| ServiceError::SolverPanic)??
        };

        let mut routes = Vec::with_capacity(solution.routes.len());
        for route in &solution.routes {
            let geom = match geometry {
                GeometryFormat::None => None,
                // GeoJson currently behaves like Polyline: the router returns
                // engine-native geometry; conversion is future work (documented
                // at the DTO/OpenAPI layer).
                GeometryFormat::Polyline | GeometryFormat::GeoJson => {
                    let vehicle = problem
                        .vehicles()
                        .iter()
                        .find(|v| v.id == route.vehicle)
                        .expect("solver returns known ids");
                    let waypoints: Vec<Coordinate> = route
                        .stops
                        .iter()
                        .map(|s| Self::stop_coordinate(&problem, s))
                        .collect();
                    Some(self.router.directions(vehicle, &waypoints).await?)
                }
            };
            routes.push(SolvedRoute { geometry: geom });
        }
        Ok(SolveResult { solution, routes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use streamlet_core::domain::*;
    use streamlet_core::matrix::CostMatrix;
    use streamlet_core::router::{RouteGeometry, Router, RouterError};

    struct FakeRouter {
        fail_matrix: bool,
    }

    #[async_trait::async_trait]
    impl Router for FakeRouter {
        async fn matrix(
            &self,
            _v: &Vehicle,
            locations: &[Coordinate],
        ) -> Result<CostMatrix, RouterError> {
            if self.fail_matrix {
                return Err(RouterError::Timeout);
            }
            let n = locations.len();
            let cell = |i: usize, j: usize| if i == j { 0.0 } else { 100.0 };
            let m: Vec<Vec<f64>> = (0..n)
                .map(|i| (0..n).map(|j| cell(i, j)).collect())
                .collect();
            Ok(CostMatrix::new(m.clone(), m).unwrap())
        }

        async fn directions(
            &self,
            _v: &Vehicle,
            _w: &[Coordinate],
        ) -> Result<RouteGeometry, RouterError> {
            Ok(RouteGeometry::Polyline("shape".into()))
        }
    }

    fn problem() -> Problem {
        let vehicle = Vehicle {
            id: VehicleId::new(1),
            start: Coordinate::new(54.0, 9.0).unwrap(),
            tank: Tank::full(Liters::new(100.0).unwrap()),
            kind: VehicleKind::Car {
                width: Meters::new(2.0).unwrap(),
                height: Meters::new(2.0).unwrap(),
            },
            shift: TimeWindow::new(Time::new(0.0).unwrap(), Time::new(10_000.0).unwrap()).unwrap(),
            max_trips: None,
        };
        let depot = Depot {
            id: DepotId::new(1),
            location: Coordinate::new(54.0, 9.0).unwrap(),
        };
        let customer = Customer {
            id: CustomerId::new(1),
            location: Coordinate::new(54.01, 9.01).unwrap(),
            demand: Liters::new(10.0).unwrap(),
            service_time: Duration::new(60.0).unwrap(),
            time_window: None,
        };
        Problem::new(vec![vehicle], vec![depot], vec![customer], vec![]).unwrap()
    }

    fn service(fail_matrix: bool) -> SolveService {
        SolveService::new(
            Arc::new(FakeRouter { fail_matrix }),
            std::time::Duration::from_secs(1),
        )
    }

    #[tokio::test]
    async fn solves_and_attaches_geometry() {
        let result = service(false)
            .solve(problem(), GeometryFormat::Polyline, None)
            .await
            .unwrap();
        assert_eq!(result.routes.len(), 1);
        assert!(matches!(
            result.routes[0].geometry,
            Some(RouteGeometry::Polyline(_))
        ));
        assert!(result.solution.unserved.is_empty());
    }

    #[tokio::test]
    async fn skips_geometry_when_not_requested() {
        let result = service(false)
            .solve(problem(), GeometryFormat::None, None)
            .await
            .unwrap();
        assert!(result.routes[0].geometry.is_none());
    }

    #[tokio::test]
    async fn client_time_limit_is_clamped_to_server_max() {
        // 10s requested, 1s configured -> must not extend the budget beyond 1s.
        let service = service(false);
        let long = Some(std::time::Duration::from_secs(10));
        let started = std::time::Instant::now();
        service
            .solve(problem(), GeometryFormat::None, long)
            .await
            .unwrap();
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    #[tokio::test]
    async fn propagates_router_errors() {
        let err = service(true)
            .solve(problem(), GeometryFormat::None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, ServiceError::Router(RouterError::Timeout)));
    }
}
