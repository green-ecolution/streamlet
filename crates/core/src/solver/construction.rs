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
        let bases: Vec<f64> = routes
            .iter()
            .map(|r| instance.evaluate(r.vehicle, &r.visits).travel_time)
            .collect();
        // Pick the (customer, position) pair with the globally cheapest feasible insertion.
        // Ties (strict `<`) keep the earliest-evaluated candidate, i.e. the customer's
        // current position in `unassigned` decides among equal-cost insertions.
        let mut best: Option<(usize, Insertion)> = None;
        for (ci, &customer) in unassigned.iter().enumerate() {
            if let Some(ins) = best_insertion(instance, &routes, &bases, customer) {
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
    bases: &[f64],
    customer: usize,
) -> Option<Insertion> {
    let mut best: Option<Insertion> = None;
    for (ri, route) in routes.iter().enumerate() {
        let base = bases[ri];
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
    use crate::domain::*;
    use crate::matrix::CostMatrix;
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

    #[test]
    fn inserts_refill_first_for_partially_filled_tank() {
        // Tank 100 at level 10; single 90L customer -> construct must refill first.
        let base = problem_small_tank_with_refill();
        let vehicle = Vehicle {
            tank: Tank::new(Liters::new(100.0).unwrap(), Liters::new(10.0).unwrap()).unwrap(),
            ..base.vehicles()[0]
        };
        let customers = vec![Customer {
            demand: Liters::new(90.0).unwrap(),
            ..base.customers()[0]
        }];
        let problem = Problem::new(
            vec![vehicle],
            base.depots().to_vec(),
            customers,
            base.refill_stations().to_vec(),
        )
        .unwrap();
        // Node ids: 0 vehicle, 1 depot, 2 customer, 3 refill.
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        let plan = construct(&instance);
        assert!(plan.unserved.is_empty());
        assert_eq!(
            plan.routes[0].visits,
            vec![3, 2],
            "refill must precede the customer"
        );
    }

    #[test]
    fn oversized_demand_stays_unserved_even_with_refill_available() {
        // 500L demand > 100L capacity: no refill can help a single visit.
        let base = problem_oversized_demand();
        let refill = RefillStation {
            id: RefillStationId::new(1),
            location: Coordinate::new(54.0, 9.0).unwrap(),
            refill_duration: Duration::new(120.0).unwrap(),
        };
        let problem = Problem::new(
            base.vehicles().to_vec(),
            base.depots().to_vec(),
            base.customers().to_vec(),
            vec![refill],
        )
        .unwrap();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        let plan = construct(&instance);
        assert_eq!(plan.unserved.len(), 1);
    }

    #[test]
    fn customers_go_to_the_nearer_vehicle() {
        // Two vehicles on a line at 0 and 100; customers at 1 and 99; depot at 50.
        // Node ids: 0,1 vehicles; 2 depot; 3,4 customers.
        let base = problem_two_customers();
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
        let pos: [f64; 5] = [0.0, 100.0, 50.0, 1.0, 99.0];
        let time: Vec<Vec<f64>> = pos
            .iter()
            .map(|a| pos.iter().map(|b| (a - b).abs()).collect())
            .collect();
        let matrix = CostMatrix::new(time.clone(), time).unwrap();
        let instance = Instance::new(&problem, &matrix).unwrap();
        let plan = construct(&instance);
        assert!(plan.unserved.is_empty());
        assert_eq!(
            plan.routes[0].visits,
            vec![3],
            "vehicle at 0 serves customer at 1"
        );
        assert_eq!(
            plan.routes[1].visits,
            vec![4],
            "vehicle at 100 serves customer at 99"
        );
    }
}
