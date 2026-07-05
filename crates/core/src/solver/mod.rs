pub mod construction;
pub mod route;
pub mod search;
pub mod segments;

use crate::domain::solution::Route;
use crate::domain::{Duration, Meters, Problem, Solution};
use crate::matrix::CostMatrix;
use route::Instance;
use search::SearchLimits;

/// Options controlling the solver's search behaviour.
#[derive(Debug, Clone, Default)]
pub struct SolveOptions {
    pub limits: SearchLimits,
}

#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    #[error(transparent)]
    Instance(#[from] route::InstanceError),
    #[error("solver produced non-finite metric: {0}")]
    Metric(f64),
}

/// Solves a VRP instance: constructs an initial plan, improves it, and maps
/// the result back to the domain `Solution`.
///
/// Deterministic for a given `problem`, `matrix`, and `options` (search runs
/// to a fixed point or a fixed iteration/time budget, never randomised).
/// Customers that cannot be placed feasibly are reported in
/// `Solution::unserved`, not as an error.
pub fn solve(
    problem: &Problem,
    matrix: &CostMatrix,
    options: &SolveOptions,
) -> Result<Solution, SolverError> {
    let instance = Instance::new(problem, matrix)?;
    let mut plan = construction::construct(&instance);
    search::improve(&instance, &mut plan, &options.limits);

    let mut routes = Vec::new();
    let (mut total_time, mut total_distance) = (0.0, 0.0);
    let main_depot = instance.depot_range().start;
    for r in &plan.routes {
        if r.visits.is_empty() {
            continue;
        }
        let eval = instance.evaluate(r.vehicle, &r.visits);
        total_time += eval.travel_time;
        total_distance += eval.distance;
        let mut stops = vec![instance.stop(r.vehicle)];
        stops.extend(r.visits.iter().map(|&id| instance.stop(id)));
        stops.push(instance.stop(main_depot));
        let service: f64 = r
            .visits
            .iter()
            .map(|&id| instance.node(id).service_time)
            .sum();
        let wait = f64::max(eval.duration.duration() - eval.travel_time - service, 0.0);
        routes.push(Route {
            vehicle: problem.vehicles()[r.vehicle].id,
            stops,
            distance: to_meters(eval.distance)?,
            travel_time: to_duration(eval.travel_time)?,
            wait_time: to_duration(wait)?,
        });
    }
    let unserved = plan
        .unserved
        .iter()
        .map(|&id| problem.customers()[id - instance.customer_range().start].id)
        .collect();
    Ok(Solution {
        routes,
        unserved,
        total_distance: to_meters(total_distance)?,
        total_travel_time: to_duration(total_time)?,
    })
}

fn to_meters(v: f64) -> Result<Meters, SolverError> {
    Meters::new(v).map_err(|_| SolverError::Metric(v))
}

fn to_duration(v: f64) -> Result<Duration, SolverError> {
    Duration::new(v).map_err(|_| SolverError::Metric(v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::route::tests::{
        problem_small_tank_with_refill, problem_two_customers, uniform_matrix,
    };

    #[test]
    fn solves_and_maps_to_domain_solution() {
        let problem = problem_two_customers();
        let solution = solve(&problem, &uniform_matrix(4), &SolveOptions::default()).unwrap();
        assert_eq!(solution.routes.len(), 1);
        let stops = &solution.routes[0].stops;
        assert!(matches!(
            stops.first(),
            Some(crate::domain::Stop::VehicleStart(_))
        ));
        assert!(matches!(stops.last(), Some(crate::domain::Stop::Depot(_))));
        // 2 customers between start and depot
        assert_eq!(stops.len(), 4);
        assert!(solution.unserved.is_empty());
        assert_eq!(solution.total_travel_time.get(), 300.0);
    }

    #[test]
    fn refill_appears_in_stop_sequence() {
        let problem = problem_small_tank_with_refill();
        let solution = solve(&problem, &uniform_matrix(5), &SolveOptions::default()).unwrap();
        let has_refill = solution.routes[0]
            .stops
            .iter()
            .any(|s| matches!(s, crate::domain::Stop::Refill(_)));
        assert!(has_refill);
    }

    #[test]
    fn rejects_matrix_of_wrong_size() {
        let problem = problem_two_customers();
        assert!(matches!(
            solve(&problem, &uniform_matrix(3), &SolveOptions::default()),
            Err(SolverError::Instance(_))
        ));
    }

    #[test]
    fn identical_input_yields_identical_output() {
        let problem = problem_small_tank_with_refill();
        let a = solve(&problem, &uniform_matrix(5), &SolveOptions::default()).unwrap();
        let b = solve(&problem, &uniform_matrix(5), &SolveOptions::default()).unwrap();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }
}
