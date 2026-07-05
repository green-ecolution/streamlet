use crate::solver::route::Instance;

/// One vehicle's visit sequence (customer/refill node ids, no start/end depot).
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleRoute {
    pub vehicle: usize,
    pub visits: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub routes: Vec<VehicleRoute>,
    /// Customer node ids that could not be placed feasibly.
    pub unserved: Vec<usize>,
}

/// Best position found for inserting a node into one route.
struct Insertion {
    route: usize,
    position: usize,
    /// Refill node to insert directly before the customer, if needed.
    refill_before: Option<usize>,
    cost_delta: f64,
}

/// Greedy best-insertion construction heuristic.
///
/// Repeatedly inserts the globally cheapest feasible (customer, position)
/// pair across all routes. If a customer does not fit directly, each refill
/// station is tried inserted directly before it. Customers that fit nowhere
/// end up in `Plan::unserved`.
pub fn construct(instance: &Instance) -> Plan {
    let n_vehicles = instance.problem().vehicles().len();
    let mut routes: Vec<VehicleRoute> = (0..n_vehicles)
        .map(|vehicle| VehicleRoute {
            vehicle,
            visits: vec![],
        })
        .collect();
    let mut unassigned: Vec<usize> = instance.customer_range().collect();
    let mut unserved = vec![];

    while !unassigned.is_empty() {
        // Pick the (customer, position) pair with the globally cheapest feasible insertion.
        let mut best: Option<(usize, Insertion)> = None;
        for (ci, &customer) in unassigned.iter().enumerate() {
            if let Some(ins) = best_insertion(instance, &routes, customer) {
                let better = best
                    .as_ref()
                    .is_none_or(|(_, b)| ins.cost_delta < b.cost_delta);
                if better {
                    best = Some((ci, ins));
                }
            }
        }
        match best {
            Some((ci, ins)) => {
                let customer = unassigned.swap_remove(ci);
                let visits = &mut routes[ins.route].visits;
                if let Some(refill) = ins.refill_before {
                    visits.insert(ins.position, refill);
                    visits.insert(ins.position + 1, customer);
                } else {
                    visits.insert(ins.position, customer);
                }
            }
            None => {
                unserved.append(&mut unassigned);
            }
        }
    }
    unserved.sort_unstable();
    Plan { routes, unserved }
}

fn best_insertion(
    instance: &Instance,
    routes: &[VehicleRoute],
    customer: usize,
) -> Option<Insertion> {
    let mut best: Option<Insertion> = None;
    for (ri, route) in routes.iter().enumerate() {
        let base = instance.evaluate(route.vehicle, &route.visits).travel_time;
        for pos in 0..=route.visits.len() {
            let mut candidate = route.visits.clone();
            candidate.insert(pos, customer);
            let eval = instance.evaluate(route.vehicle, &candidate);
            let mut found: Option<(f64, Option<usize>)> =
                eval.is_feasible.then_some((eval.travel_time - base, None));
            if found.is_none() {
                // Retry with each refill station inserted before the customer.
                for refill in instance.refill_range() {
                    let mut with_refill = route.visits.clone();
                    with_refill.insert(pos, refill);
                    with_refill.insert(pos + 1, customer);
                    let eval = instance.evaluate(route.vehicle, &with_refill);
                    if eval.is_feasible {
                        let delta = eval.travel_time - base;
                        if found.as_ref().is_none_or(|(d, _)| delta < *d) {
                            found = Some((delta, Some(refill)));
                        }
                    }
                }
            }
            if let Some((delta, refill_before)) = found
                && best.as_ref().is_none_or(|b| delta < b.cost_delta)
            {
                best = Some(Insertion {
                    route: ri,
                    position: pos,
                    refill_before,
                    cost_delta: delta,
                });
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::route::Instance;
    use crate::solver::route::tests::{
        problem_oversized_demand, problem_small_tank_with_refill, problem_two_customers,
        uniform_matrix,
    };

    #[test]
    fn assigns_all_customers_when_capacity_suffices() {
        let problem = problem_two_customers();
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        let plan = construct(&instance);
        assert!(plan.unserved.is_empty());
        assert_eq!(plan.routes[0].visits.len(), 2);
        assert!(instance.evaluate(0, &plan.routes[0].visits).is_feasible);
    }

    #[test]
    fn inserts_refill_when_tank_too_small() {
        // 2 customers à 40L, tank 50L, one refill station -> must visit refill between them
        let problem = problem_small_tank_with_refill();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        let plan = construct(&instance);
        assert!(plan.unserved.is_empty());
        let visits = &plan.routes[0].visits;
        assert!(
            visits.iter().any(|id| instance.refill_range().contains(id)),
            "expected a refill visit in {visits:?}"
        );
        assert!(instance.evaluate(0, visits).is_feasible);
    }

    #[test]
    fn reports_unservable_customers() {
        // demand larger than tank capacity and no refill exists
        let problem = problem_oversized_demand();
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        let plan = construct(&instance);
        assert_eq!(plan.unserved.len(), 1);
    }
}
