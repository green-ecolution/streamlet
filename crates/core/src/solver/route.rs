use std::ops::Range;

use crate::domain::{Problem, Stop};
use crate::matrix::{CostMatrix, NodeIndex};
use crate::solver::segments::{DurationSegment, LoadSegment};

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("matrix has {matrix} nodes but problem has {problem}")]
    SizeMismatch { matrix: usize, problem: usize },
}

/// Node data flattened for the solver. `demand` is 0 for non-customers.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SolverNode {
    pub demand: f64,
    pub service_time: f64,
    pub tw_early: f64,
    pub tw_late: f64,
    pub is_refill: bool,
}

/// A solve-ready view of a problem: flat nodes + matrix + vehicle data.
///
/// Owns a copy of the matrix rather than borrowing it: callers routinely build
/// the matrix inline (`Instance::new(&problem, &build_matrix(..))`), and a
/// borrow tied to the same lifetime as `problem` would dangle once that
/// temporary is dropped at the end of the statement.
pub struct Instance<'a> {
    problem: &'a Problem,
    matrix: CostMatrix,
    nodes: Vec<SolverNode>,
    depots: Range<usize>,
    customers: Range<usize>,
    refills: Range<usize>,
}

/// Result of evaluating one vehicle's visit sequence.
#[derive(Debug, Clone, Copy)]
pub struct RouteEval {
    pub travel_time: f64,
    pub distance: f64,
    pub load: LoadSegment,
    pub duration: DurationSegment,
    pub is_feasible: bool,
    /// Number of trips (1 + number of refill visits).
    pub trips: u32,
}

impl<'a> Instance<'a> {
    pub fn new(problem: &'a Problem, matrix: &CostMatrix) -> Result<Self, InstanceError> {
        let n_vehicles = problem.vehicles().len();
        let n = n_vehicles
            + problem.depots().len()
            + problem.customers().len()
            + problem.refill_stations().len();
        if matrix.len() != n {
            return Err(InstanceError::SizeMismatch {
                matrix: matrix.len(),
                problem: n,
            });
        }
        let mut nodes = Vec::with_capacity(n);
        for v in problem.vehicles() {
            nodes.push(SolverNode {
                demand: 0.0,
                service_time: 0.0,
                tw_early: v.shift.start().get(),
                tw_late: v.shift.end().get(),
                is_refill: false,
            });
        }
        let depots = nodes.len()..nodes.len() + problem.depots().len();
        for _ in problem.depots() {
            nodes.push(SolverNode {
                demand: 0.0,
                service_time: 0.0,
                tw_early: 0.0,
                tw_late: f64::INFINITY,
                is_refill: false,
            });
        }
        let customers = nodes.len()..nodes.len() + problem.customers().len();
        for c in problem.customers() {
            let (early, late) = c
                .time_window
                .map(|tw| (tw.start().get(), tw.end().get()))
                .unwrap_or((0.0, f64::INFINITY));
            nodes.push(SolverNode {
                demand: c.demand.get(),
                service_time: c.service_time.get(),
                tw_early: early,
                tw_late: late,
                is_refill: false,
            });
        }
        let refills = nodes.len()..nodes.len() + problem.refill_stations().len();
        for r in problem.refill_stations() {
            nodes.push(SolverNode {
                demand: 0.0,
                service_time: r.refill_duration.get(),
                tw_early: 0.0,
                tw_late: f64::INFINITY,
                is_refill: true,
            });
        }
        Ok(Self {
            problem,
            matrix: matrix.clone(),
            nodes,
            depots,
            customers,
            refills,
        })
    }

    pub fn problem(&self) -> &Problem {
        self.problem
    }

    pub fn matrix(&self) -> &CostMatrix {
        &self.matrix
    }

    pub fn vehicle_start(&self, vehicle: usize) -> NodeIndex {
        NodeIndex(vehicle)
    }

    pub fn depot_range(&self) -> Range<usize> {
        self.depots.clone()
    }

    pub fn customer_range(&self) -> Range<usize> {
        self.customers.clone()
    }

    pub fn refill_range(&self) -> Range<usize> {
        self.refills.clone()
    }

    pub(crate) fn node(&self, id: usize) -> SolverNode {
        self.nodes[id]
    }

    /// Maps a solver node id back to a domain `Stop`.
    pub fn stop(&self, vehicle: usize, id: usize) -> Stop {
        if id < self.depots.start {
            Stop::VehicleStart(self.problem.vehicles()[vehicle].id)
        } else if self.depots.contains(&id) {
            Stop::Depot(self.problem.depots()[id - self.depots.start].id)
        } else if self.customers.contains(&id) {
            Stop::Customer(self.problem.customers()[id - self.customers.start].id)
        } else {
            Stop::Refill(self.problem.refill_stations()[id - self.refills.start].id)
        }
    }

    pub fn evaluate(&self, vehicle: usize, visits: &[usize]) -> RouteEval {
        let v = &self.problem.vehicles()[vehicle];
        let capacity = v.tank.capacity().get();
        let shift_len = v.shift.end().get() - v.shift.start().get();
        let start_level = v.tank.level().get();
        let main_depot = self.depots.start;

        // Tank may start partially filled: model the gap as an initial virtual demand.
        let mut load = LoadSegment::from_customer(capacity - start_level);
        let mut dur = DurationSegment::from_node(v.shift.start().get(), v.shift.end().get(), 0.0);
        let (mut travel_time, mut distance, mut trips) = (0.0, 0.0, 1u32);
        let mut prev = self.vehicle_start(vehicle);

        for &id in visits.iter().chain(std::iter::once(&main_depot)) {
            let node = if id == main_depot {
                SolverNode {
                    demand: 0.0,
                    service_time: 0.0,
                    tw_early: 0.0,
                    tw_late: f64::INFINITY,
                    is_refill: false,
                }
            } else {
                self.node(id)
            };
            let next = NodeIndex(id);
            let edge = self.matrix.travel_time(prev, next);
            travel_time += edge;
            distance += self.matrix.distance(prev, next);
            dur = DurationSegment::merge(
                dur,
                DurationSegment::from_node(node.tw_early, node.tw_late, node.service_time),
                edge,
            );
            if node.is_refill {
                // A reload is exactly finalize: capture this trip's excess, reset the load.
                load = load.finalize(capacity);
                dur = dur.finalise_back();
                trips += 1;
            } else {
                load = LoadSegment::merge(load, LoadSegment::from_customer(node.demand));
            }
            prev = next;
        }

        let max_trips = v.max_trips.map(|t| t.get()).unwrap_or(u32::MAX);
        let is_feasible =
            load.is_feasible(capacity) && dur.is_feasible_with_max(shift_len) && trips <= max_trips;
        RouteEval {
            travel_time,
            distance,
            load,
            duration: dur,
            is_feasible,
            trips,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::domain::*;
    use crate::matrix::CostMatrix;

    // 4 nodes: 0 = vehicle start, 1 = depot, 2..3 = customers. Symmetric, 100s/1km apart.
    pub(crate) fn problem_two_customers() -> Problem {
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
        let customer = |id, demand| Customer {
            id: CustomerId::new(id),
            location: Coordinate::new(54.0, 9.0).unwrap(),
            demand: Liters::new(demand).unwrap(),
            service_time: Duration::new(60.0).unwrap(),
            time_window: None,
        };
        Problem::new(
            vec![vehicle],
            vec![depot],
            vec![customer(1, 40.0), customer(2, 40.0)],
            vec![],
        )
        .unwrap()
    }

    pub(crate) fn uniform_matrix(n: usize) -> CostMatrix {
        let cell = |i: usize, j: usize| if i == j { 0.0 } else { 100.0 };
        let time = (0..n)
            .map(|i| (0..n).map(|j| cell(i, j)).collect())
            .collect();
        let dist = (0..n)
            .map(|i| (0..n).map(|j| cell(i, j) * 10.0).collect())
            .collect();
        CostMatrix::new(time, dist).unwrap()
    }

    pub(crate) fn problem_two_customers_small_tank() -> Problem {
        // Same as problem_two_customers but the tank holds only 50L: 40+40 must violate.
        let base = problem_two_customers();
        let vehicle = Vehicle {
            tank: Tank::full(Liters::new(50.0).unwrap()),
            ..base.vehicles()[0]
        };
        Problem::new(
            vec![vehicle],
            base.depots().to_vec(),
            base.customers().to_vec(),
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn instance_indexes_nodes_in_field_order() {
        let problem = problem_two_customers();
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        assert_eq!(instance.vehicle_start(0).0, 0);
        assert_eq!(instance.depot_range(), 1..2);
        assert_eq!(instance.customer_range(), 2..4);
        assert_eq!(instance.refill_range(), 4..4);
    }

    #[test]
    fn instance_rejects_matrix_size_mismatch() {
        let problem = problem_two_customers();
        assert!(Instance::new(&problem, &uniform_matrix(3)).is_err());
    }

    #[test]
    fn evaluates_feasible_route() {
        let problem = problem_two_customers();
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        // vehicle start -> c2 -> c3 -> depot: 3 legs à 100s + 2×60s service
        let eval = instance.evaluate(0, &[2, 3]);
        assert!(eval.is_feasible);
        assert_eq!(eval.travel_time, 300.0);
        assert_eq!(eval.duration.duration(), 420.0);
        assert_eq!(eval.distance, 3000.0);
    }

    #[test]
    fn detects_capacity_violation_without_refill() {
        let problem = problem_two_customers(); // 40 + 40 fits in 100
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        assert!(instance.evaluate(0, &[2, 3]).is_feasible);

        let small = problem_two_customers_small_tank(); // 40 + 40 exceeds 50
        let instance = Instance::new(&small, &uniform_matrix(4)).unwrap();
        assert!(!instance.evaluate(0, &[2, 3]).is_feasible);
    }
}
