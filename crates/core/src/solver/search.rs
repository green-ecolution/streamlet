use std::time::{Duration as StdDuration, Instant};

use crate::solver::construction::Plan;
use crate::solver::route::Instance;

#[derive(Debug, Clone)]
pub struct SearchLimits {
    /// Max number of inter-route improvement rounds, each preceded by a full
    /// intra-route descent. Not a cap on individual moves.
    pub max_iterations: u32,
    /// Wall-clock budget for the whole search. Checked between moves, so a
    /// single operator scan may overrun it slightly.
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
/// Deterministic provided the time limit is not hit; `max_iterations` bounds
/// the number of inter-route improvement rounds (each preceded by a full
/// intra-route descent), not individual moves.
pub fn improve(instance: &Instance, plan: &mut Plan, limits: &SearchLimits) {
    let deadline = Instant::now() + limits.time_limit;
    for _ in 0..limits.max_iterations {
        // Full intra-route descent to a local optimum of the cheap operators.
        while Instant::now() < deadline
            && (relocate(instance, plan) || swap(instance, plan) || two_opt(instance, plan))
        {
        }
        if Instant::now() >= deadline {
            return;
        }
        if !improve_inter(instance, plan) {
            return; // local optimum of all operators
        }
    }
}

/// Ordering used by all operators: feasibility first, then travel time.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Score {
    infeasible: bool,
    travel_time: f64,
}

impl Score {
    fn of(instance: &Instance, vehicle: usize, visits: &[usize]) -> Self {
        let eval = instance.evaluate(vehicle, visits);
        Self {
            infeasible: !eval.is_feasible,
            travel_time: eval.travel_time,
        }
    }

    fn better_than(self, other: Self) -> bool {
        match (self.infeasible, other.infeasible) {
            (false, true) => true,
            (true, false) => false,
            _ => self.travel_time < other.travel_time - EPSILON,
        }
    }
}

/// Applies `mutate` to a scratch copy; keeps it if strictly better and not
/// newly infeasible (an already-infeasible route may stay infeasible while
/// its travel time improves).
fn try_move(
    instance: &Instance,
    plan: &mut Plan,
    route: usize,
    mutate: impl FnOnce(&mut Vec<usize>),
) -> bool {
    let vehicle = plan.routes[route].vehicle;
    let before = Score::of(instance, vehicle, &plan.routes[route].visits);
    let mut candidate = plan.routes[route].visits.clone();
    mutate(&mut candidate);
    let after = Score::of(instance, vehicle, &candidate);
    if after.better_than(before) && (!after.infeasible || before.infeasible) {
        plan.routes[route].visits = candidate;
        true
    } else {
        false
    }
}

fn improve_inter(instance: &Instance, plan: &mut Plan) -> bool {
    relocate_between(instance, plan)
        || two_opt_star(instance, plan)
        || cross_exchange(instance, plan)
        || reposition_refills(instance, plan)
}

/// Applies `mutate` to scratch copies of two routes; keeps them if the summed
/// score improves and neither route got newly infeasible.
fn try_pair_move(
    instance: &Instance,
    plan: &mut Plan,
    (a, b): (usize, usize),
    mutate: impl FnOnce(&mut Vec<usize>, &mut Vec<usize>),
) -> bool {
    let (va, vb) = (plan.routes[a].vehicle, plan.routes[b].vehicle);
    let before_a = Score::of(instance, va, &plan.routes[a].visits);
    let before_b = Score::of(instance, vb, &plan.routes[b].visits);
    let mut ca = plan.routes[a].visits.clone();
    let mut cb = plan.routes[b].visits.clone();
    mutate(&mut ca, &mut cb);
    let after_a = Score::of(instance, va, &ca);
    let after_b = Score::of(instance, vb, &cb);
    let feasibility_ok = (!after_a.infeasible || before_a.infeasible)
        && (!after_b.infeasible || before_b.infeasible);
    let combined_before = Score {
        infeasible: before_a.infeasible || before_b.infeasible,
        travel_time: before_a.travel_time + before_b.travel_time,
    };
    let combined_after = Score {
        infeasible: after_a.infeasible || after_b.infeasible,
        travel_time: after_a.travel_time + after_b.travel_time,
    };
    if feasibility_ok && combined_after.better_than(combined_before) {
        plan.routes[a].visits = ca;
        plan.routes[b].visits = cb;
        true
    } else {
        false
    }
}

/// Move one visit from route a to any position in route b.
fn relocate_between(instance: &Instance, plan: &mut Plan) -> bool {
    for a in 0..plan.routes.len() {
        for b in 0..plan.routes.len() {
            if a == b {
                continue;
            }
            for from in 0..plan.routes[a].visits.len() {
                for to in 0..=plan.routes[b].visits.len() {
                    if try_pair_move(instance, plan, (a, b), |va, vb| {
                        let node = va.remove(from);
                        vb.insert(to, node);
                    }) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Swap route tails: a[i..] <-> b[j..].
fn two_opt_star(instance: &Instance, plan: &mut Plan) -> bool {
    for a in 0..plan.routes.len() {
        for b in a + 1..plan.routes.len() {
            for i in 0..=plan.routes[a].visits.len() {
                for j in 0..=plan.routes[b].visits.len() {
                    if try_pair_move(instance, plan, (a, b), |va, vb| {
                        let tail_a: Vec<usize> = va.split_off(i);
                        let tail_b: Vec<usize> = vb.split_off(j);
                        va.extend(tail_b);
                        vb.extend(tail_a);
                    }) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Exchange one visit of route a with one visit of route b.
fn cross_exchange(instance: &Instance, plan: &mut Plan) -> bool {
    for a in 0..plan.routes.len() {
        for b in a + 1..plan.routes.len() {
            for i in 0..plan.routes[a].visits.len() {
                for j in 0..plan.routes[b].visits.len() {
                    if try_pair_move(instance, plan, (a, b), |va, vb| {
                        std::mem::swap(&mut va[i], &mut vb[j]);
                    }) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Remove every refill visit and re-insert each at its best position (or drop it).
/// Accepts infeasible→feasible transitions, so it repairs broken refill placements.
fn reposition_refills(instance: &Instance, plan: &mut Plan) -> bool {
    for r in 0..plan.routes.len() {
        let vehicle = plan.routes[r].vehicle;
        let refill_positions: Vec<usize> = plan.routes[r]
            .visits
            .iter()
            .enumerate()
            .filter(|(_, id)| instance.refill_range().contains(id))
            .map(|(pos, _)| pos)
            .collect();
        for &pos in &refill_positions {
            let before = Score::of(instance, vehicle, &plan.routes[r].visits);
            let refill = plan.routes[r].visits[pos];
            let mut without: Vec<usize> = plan.routes[r].visits.clone();
            without.remove(pos);
            // Candidate: drop entirely, or re-insert at every other position.
            let mut best = (Score::of(instance, vehicle, &without), without.clone());
            for to in 0..=without.len() {
                let mut candidate = without.clone();
                candidate.insert(to, refill);
                let score = Score::of(instance, vehicle, &candidate);
                if score.better_than(best.0) {
                    best = (score, candidate);
                }
            }
            if best.0.better_than(before) {
                plan.routes[r].visits = best.1;
                return true;
            }
        }
    }
    false
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

    // Two vehicles; two customer clusters on a line; each vehicle starts inside one cluster.
    // Optimal: each vehicle serves its own cluster.
    fn two_cluster_setup() -> (Problem, CostMatrix) {
        let vehicle = |id| Vehicle {
            id: VehicleId::new(id),
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
        let problem = Problem::new(
            vec![vehicle(1), vehicle(2)],
            vec![depot],
            vec![customer(1), customer(2), customer(3), customer(4)],
            vec![],
        )
        .unwrap();
        // Node ids: 0,1 vehicle starts; 2 depot; 3,4,5,6 customers.
        // 1D positions: v0 @ 0, v1 @ 100, depot @ 50, c3,c4 @ 0.5..1, c5,c6 @ 100.5..101
        let pos: [f64; 7] = [0.0, 100.0, 50.0, 0.5, 1.0, 100.5, 101.0];
        let time: Vec<Vec<f64>> = pos
            .iter()
            .map(|a| pos.iter().map(|b| (a - b).abs()).collect())
            .collect();
        let matrix = CostMatrix::new(time.clone(), time).unwrap();
        (problem, matrix)
    }

    #[test]
    fn inter_route_relocate_moves_customer_to_nearer_vehicle() {
        let (problem, matrix) = two_cluster_setup();
        let instance = Instance::new(&problem, &matrix).unwrap();
        // Bad assignment: vehicle 0 serves far-cluster customer 5, vehicle 1 serves the rest of it.
        let mut plan = Plan {
            routes: vec![
                VehicleRoute {
                    vehicle: 0,
                    visits: vec![3, 4, 5],
                },
                VehicleRoute {
                    vehicle: 1,
                    visits: vec![6],
                },
            ],
            unserved: vec![],
        };
        let before = total_time(&instance, &plan);
        improve(&instance, &mut plan, &SearchLimits::default());
        assert!(total_time(&instance, &plan) < before);
        assert!(
            !plan.routes[0].visits.contains(&5),
            "customer 5 should move to vehicle 1"
        );
    }

    #[test]
    fn search_repairs_broken_refill_placement() {
        let problem = crate::solver::route::tests::problem_small_tank_with_refill();
        let instance =
            Instance::new(&problem, &crate::solver::route::tests::uniform_matrix(5)).unwrap();
        // Refill (node 4) placed pointlessly at the very start; with tank 50 and demands
        // 40+40 the refill is needed BETWEEN the customers, not before them. Repaired by
        // intra relocate, but still a valuable regression test for the repair behaviour.
        let mut plan = Plan {
            routes: vec![VehicleRoute {
                vehicle: 0,
                visits: vec![4, 2, 3],
            }],
            unserved: vec![],
        };
        assert!(!instance.evaluate(0, &plan.routes[0].visits).is_feasible);
        improve(&instance, &mut plan, &SearchLimits::default());
        let visits = &plan.routes[0].visits;
        assert!(
            instance.evaluate(0, visits).is_feasible,
            "search must repair refill placement: {visits:?}"
        );
    }

    #[test]
    fn useless_refill_is_dropped() {
        // Tank 100 easily covers 40+40: the refill visit is pure overhead
        // (+2 legs on a uniform matrix). Only reposition_refills can DROP a
        // visit, so this pins that operator.
        let base = crate::solver::route::tests::problem_small_tank_with_refill();
        let vehicle = Vehicle {
            tank: Tank::full(Liters::new(100.0).unwrap()),
            ..base.vehicles()[0]
        };
        let problem = Problem::new(
            vec![vehicle],
            base.depots().to_vec(),
            base.customers().to_vec(),
            base.refill_stations().to_vec(),
        )
        .unwrap();
        let instance =
            Instance::new(&problem, &crate::solver::route::tests::uniform_matrix(5)).unwrap();
        let mut plan = Plan {
            routes: vec![VehicleRoute {
                vehicle: 0,
                visits: vec![2, 4, 3],
            }],
            unserved: vec![],
        };
        improve(&instance, &mut plan, &SearchLimits::default());
        assert_eq!(
            plan.routes[0].visits,
            vec![2, 3],
            "useless refill must be dropped"
        );
    }

    #[test]
    fn pair_move_must_not_break_target_feasibility() {
        // Two vehicles, tank 50 each; customers 40+40 assigned one per route.
        // Merging them into one route would cut travel time on this matrix but
        // overload the target tank — the guard must reject it.
        let base = crate::solver::route::tests::problem_two_customers_small_tank();
        let vehicle = |id| Vehicle {
            id: VehicleId::new(id),
            ..base.vehicles()[0]
        };
        let problem = Problem::new(
            vec![vehicle(1), vehicle(2)],
            base.depots().to_vec(),
            base.customers().to_vec(),
            vec![],
        )
        .unwrap();
        // Node ids: 0,1 vehicles; 2 depot; 3,4 customers.
        let instance =
            Instance::new(&problem, &crate::solver::route::tests::uniform_matrix(5)).unwrap();
        let mut plan = Plan {
            routes: vec![
                VehicleRoute {
                    vehicle: 0,
                    visits: vec![3],
                },
                VehicleRoute {
                    vehicle: 1,
                    visits: vec![4],
                },
            ],
            unserved: vec![],
        };
        improve(&instance, &mut plan, &SearchLimits::default());
        for r in &plan.routes {
            assert!(instance.evaluate(r.vehicle, &r.visits).is_feasible);
        }
        assert_eq!(plan.routes.iter().map(|r| r.visits.len()).sum::<usize>(), 2);
    }
}
