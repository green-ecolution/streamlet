use std::time::{Duration as StdDuration, Instant};

use crate::solver::construction::Plan;
use crate::solver::route::Instance;

#[derive(Debug, Clone)]
pub struct SearchLimits {
    pub max_iterations: u32,
    pub time_limit: StdDuration,
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            time_limit: StdDuration::from_secs(2),
        }
    }
}

const EPSILON: f64 = 1e-9;

/// Deterministic VND-style descent: cheap intra-route operators first, the
/// expensive inter-route operators only once the cheap ones are exhausted.
pub fn improve(instance: &Instance, plan: &mut Plan, limits: &SearchLimits) {
    let deadline = Instant::now() + limits.time_limit;
    for _ in 0..limits.max_iterations {
        if Instant::now() >= deadline {
            return;
        }
        let improved_intra =
            relocate(instance, plan) || swap(instance, plan) || two_opt(instance, plan);
        if improved_intra {
            continue; // restart with cheap operators
        }
        if !improve_inter(instance, plan) {
            return; // local optimum
        }
    }
}

/// Placeholder until Task 8 adds inter-route operators.
fn improve_inter(_instance: &Instance, _plan: &mut Plan) -> bool {
    false
}

fn route_time(instance: &Instance, vehicle: usize, visits: &[usize]) -> f64 {
    instance.evaluate(vehicle, visits).travel_time
}

/// Applies `mutate` to a scratch copy; keeps it if strictly better and feasible.
///
/// Whole-route evaluation per candidate is the accepted approach for now;
/// O(1) segment deltas are future work.
fn try_move(
    instance: &Instance,
    plan: &mut Plan,
    route: usize,
    mutate: impl FnOnce(&mut Vec<usize>),
) -> bool {
    let vehicle = plan.routes[route].vehicle;
    let before = route_time(instance, vehicle, &plan.routes[route].visits);
    let mut candidate = plan.routes[route].visits.clone();
    mutate(&mut candidate);
    let eval = instance.evaluate(vehicle, &candidate);
    if eval.is_feasible && eval.travel_time < before - EPSILON {
        plan.routes[route].visits = candidate;
        true
    } else {
        false
    }
}

/// Move one visit to another position in the same route.
fn relocate(instance: &Instance, plan: &mut Plan) -> bool {
    for r in 0..plan.routes.len() {
        let len = plan.routes[r].visits.len();
        for from in 0..len {
            for to in 0..len {
                if from == to {
                    continue;
                }
                if try_move(instance, plan, r, |v| {
                    let node = v.remove(from);
                    v.insert(to, node);
                }) {
                    return true;
                }
            }
        }
    }
    false
}

/// Exchange two visits within the same route.
fn swap(instance: &Instance, plan: &mut Plan) -> bool {
    for r in 0..plan.routes.len() {
        let len = plan.routes[r].visits.len();
        for i in 0..len {
            for j in i + 1..len {
                if try_move(instance, plan, r, |v| v.swap(i, j)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Reverse a subsequence (classic 2-opt).
fn two_opt(instance: &Instance, plan: &mut Plan) -> bool {
    for r in 0..plan.routes.len() {
        let len = plan.routes[r].visits.len();
        for i in 0..len {
            for j in i + 2..=len {
                if try_move(instance, plan, r, |v| v[i..j].reverse()) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::*;
    use crate::matrix::CostMatrix;
    use crate::solver::construction::{Plan, VehicleRoute};
    use crate::solver::route::Instance;

    // Line topology: depot(1) - c2 - c3 - c4 on a line; visiting in order 2,3,4 is optimal.
    fn line_matrix() -> CostMatrix {
        // nodes: 0 vehicle start (= depot), 1 depot, 2,3,4 customers at distance 1,2,3
        let pos: [f64; 5] = [0.0, 0.0, 1.0, 2.0, 3.0];
        let time: Vec<Vec<f64>> = pos
            .iter()
            .map(|a| pos.iter().map(|b| (a - b).abs() * 100.0).collect())
            .collect();
        let dist = time.clone();
        CostMatrix::new(time, dist).unwrap()
    }

    fn line_problem() -> Problem {
        let vehicle = Vehicle {
            id: VehicleId::new(1),
            start: Coordinate::new(54.0, 9.0).unwrap(),
            tank: Tank::full(Liters::new(1000.0).unwrap()),
            kind: VehicleKind::Car {
                width: Meters::new(2.0).unwrap(),
                height: Meters::new(2.0).unwrap(),
            },
            shift: TimeWindow::new(Time::new(0.0).unwrap(), Time::new(100_000.0).unwrap()).unwrap(),
            max_trips: None,
        };
        let depot = Depot {
            id: DepotId::new(1),
            location: Coordinate::new(54.0, 9.0).unwrap(),
        };
        let customer = |id| Customer {
            id: CustomerId::new(id),
            location: Coordinate::new(54.0, 9.0).unwrap(),
            demand: Liters::new(1.0).unwrap(),
            service_time: Duration::new(0.0).unwrap(),
            time_window: None,
        };
        Problem::new(
            vec![vehicle],
            vec![depot],
            vec![customer(1), customer(2), customer(3)],
            vec![],
        )
        .unwrap()
    }

    fn total_time(instance: &Instance, plan: &Plan) -> f64 {
        plan.routes
            .iter()
            .map(|r| instance.evaluate(r.vehicle, &r.visits).travel_time)
            .sum()
    }

    #[test]
    fn two_opt_untangles_bad_visit_order() {
        let problem = line_problem();
        let matrix = line_matrix();
        let instance = Instance::new(&problem, &matrix).unwrap();
        // Deliberately bad order: 3, 2, 4 (node ids: customers are 2,3,4)
        let mut plan = Plan {
            routes: vec![VehicleRoute {
                vehicle: 0,
                visits: vec![3, 2, 4],
            }],
            unserved: vec![],
        };
        let before = total_time(&instance, &plan);
        improve(&instance, &mut plan, &SearchLimits::default());
        let after = total_time(&instance, &plan);
        assert!(after < before);
        assert_eq!(plan.routes[0].visits, vec![2, 3, 4]);
    }

    #[test]
    fn search_never_returns_infeasible_or_worse_plan() {
        let problem = line_problem();
        let matrix = line_matrix();
        let instance = Instance::new(&problem, &matrix).unwrap();
        let mut plan = crate::solver::construction::construct(&instance);
        let before = total_time(&instance, &plan);
        improve(&instance, &mut plan, &SearchLimits::default());
        assert!(total_time(&instance, &plan) <= before);
        for r in &plan.routes {
            assert!(r.visits.is_empty() || instance.evaluate(r.vehicle, &r.visits).is_feasible);
        }
    }

    #[test]
    fn improve_is_deterministic() {
        let problem = line_problem();
        let matrix = line_matrix();
        let instance = Instance::new(&problem, &matrix).unwrap();
        let mut a = Plan {
            routes: vec![VehicleRoute {
                vehicle: 0,
                visits: vec![4, 2, 3],
            }],
            unserved: vec![],
        };
        let mut b = a.clone();
        improve(&instance, &mut a, &SearchLimits::default());
        improve(&instance, &mut b, &SearchLimits::default());
        assert_eq!(a, b);
    }
}
