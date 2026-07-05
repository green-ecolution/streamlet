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
    pub fn stop(&self, id: usize) -> Stop {
        if id < self.depots.start {
            Stop::VehicleStart(self.problem.vehicles()[id].id)
        } else if self.depots.contains(&id) {
            Stop::Depot(self.problem.depots()[id - self.depots.start].id)
        } else if self.customers.contains(&id) {
            Stop::Customer(self.problem.customers()[id - self.customers.start].id)
        } else {
            Stop::Refill(self.problem.refill_stations()[id - self.refills.start].id)
        }
    }

    /// Evaluates one vehicle's visit sequence.
    ///
    /// `visits` holds customer and refill node ids only: the vehicle start and
    /// the final return to the main depot are implicit and appended
    /// automatically. Any *other* depot id that appears in `visits` is treated
    /// as a plain zero-demand, zero-service node, not a trip boundary — only
    /// refill stations trigger a reload/finalize. Consequently a refill
    /// visited before the first customer still counts as a completed trip
    /// (`trips = 1 + number of refill visits`), even though nothing was
    /// delivered yet, by design.
    ///
    /// `RouteEval.load.delivery` includes the initial virtual demand
    /// `capacity - level` used to model a partially-filled tank; it is not the
    /// literal number of liters handed to customers.
    ///
    /// Panics if `vehicle` is out of range for the problem's vehicles, or if
    /// any id in `visits` is out of range for the solver's node table.
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

        // The final leg always returns to the main depot, but that return must
        // still respect the vehicle's shift window: an infinite `tw_late` here
        // would let the vehicle arrive back arbitrarily late without penalty.
        let return_node = SolverNode {
            demand: 0.0,
            service_time: 0.0,
            tw_early: v.shift.start().get(),
            tw_late: v.shift.end().get(),
            is_refill: false,
        };

        for &id in visits.iter().chain(std::iter::once(&main_depot)) {
            let node = if id == main_depot {
                return_node
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

    pub(crate) fn problem_small_tank_with_refill() -> Problem {
        // Node ids: 0 vehicle, 1 depot, 2..4 customers, 4 refill. Tank 50, demands 40+40.
        let base = problem_two_customers_small_tank();
        let refill = RefillStation {
            id: RefillStationId::new(1),
            location: Coordinate::new(54.0, 9.0).unwrap(),
            refill_duration: Duration::new(120.0).unwrap(),
        };
        Problem::new(
            base.vehicles().to_vec(),
            base.depots().to_vec(),
            base.customers().to_vec(),
            vec![refill],
        )
        .unwrap()
    }

    pub(crate) fn problem_oversized_demand() -> Problem {
        let base = problem_two_customers();
        let big = Customer {
            demand: Liters::new(500.0).unwrap(),
            ..base.customers()[0]
        };
        Problem::new(
            base.vehicles().to_vec(),
            base.depots().to_vec(),
            vec![big, base.customers()[1]],
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

    fn with_shift(problem: Problem, start: f64, end: f64) -> Problem {
        let vehicle = Vehicle {
            shift: TimeWindow::new(Time::new(start).unwrap(), Time::new(end).unwrap()).unwrap(),
            ..problem.vehicles()[0]
        };
        Problem::new(
            vec![vehicle],
            problem.depots().to_vec(),
            problem.customers().to_vec(),
            problem.refill_stations().to_vec(),
        )
        .unwrap()
    }

    fn with_customer_window(problem: Problem, idx: usize, start: f64, end: f64) -> Problem {
        let mut customers = problem.customers().to_vec();
        customers[idx] = Customer {
            time_window: Some(
                TimeWindow::new(Time::new(start).unwrap(), Time::new(end).unwrap()).unwrap(),
            ),
            ..customers[idx]
        };
        Problem::new(
            problem.vehicles().to_vec(),
            problem.depots().to_vec(),
            customers,
            problem.refill_stations().to_vec(),
        )
        .unwrap()
    }

    fn ten_second_matrix(n: usize) -> CostMatrix {
        let cell = |i: usize, j: usize| if i == j { 0.0 } else { 10.0 };
        let time: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| cell(i, j)).collect())
            .collect();
        CostMatrix::new(time.clone(), time).unwrap()
    }

    #[test]
    fn route_must_return_before_shift_end() {
        // Shift [0,200]; customer window opens at 150 -> serve until 210,
        // return at 220 > shift end 200. The route duration (80) stays below
        // the shift length, so only the return-deadline check can catch this.
        let problem = with_customer_window(
            with_shift(problem_two_customers(), 0.0, 200.0),
            0,
            150.0,
            160.0,
        );
        let instance = Instance::new(&problem, &ten_second_matrix(4)).unwrap();
        let eval = instance.evaluate(0, &[2]);
        assert!(!eval.is_feasible, "late return must be infeasible");
    }

    #[test]
    fn return_within_shift_is_feasible() {
        // Shift [0,200]; customer window [80,90], 10s legs, 60s service:
        // arrive 80, depart 140, return 150 <= 200.
        let problem = with_customer_window(
            with_shift(problem_two_customers(), 0.0, 200.0),
            0,
            80.0,
            90.0,
        );
        let instance = Instance::new(&problem, &ten_second_matrix(4)).unwrap();
        // Serve only customer 2 (node id 2); customer 3 keeps no window.
        assert!(instance.evaluate(0, &[2]).is_feasible);
    }

    #[test]
    fn empty_visits_is_feasible_start_to_depot() {
        let problem = problem_two_customers();
        let instance = Instance::new(&problem, &uniform_matrix(4)).unwrap();
        let eval = instance.evaluate(0, &[]);
        assert!(eval.is_feasible);
        assert_eq!(eval.travel_time, 100.0);
        assert_eq!(eval.trips, 1);
    }

    #[test]
    fn refill_restores_capacity_mid_route() {
        let problem = problem_small_tank_with_refill();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        assert!(
            !instance.evaluate(0, &[2, 3]).is_feasible,
            "40+40 > 50 without refill"
        );
        let eval = instance.evaluate(0, &[2, 4, 3]);
        assert!(
            eval.is_feasible,
            "refill between customers must restore capacity"
        );
        assert_eq!(eval.trips, 2);
    }

    #[test]
    fn max_trips_limits_refills() {
        let base = problem_small_tank_with_refill();
        let vehicle = Vehicle {
            max_trips: Some(std::num::NonZeroU32::new(1).unwrap()),
            ..base.vehicles()[0]
        };
        let problem = Problem::new(
            vec![vehicle],
            base.depots().to_vec(),
            base.customers().to_vec(),
            base.refill_stations().to_vec(),
        )
        .unwrap();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        assert!(
            !instance.evaluate(0, &[2, 4, 3]).is_feasible,
            "2 trips exceed max_trips 1"
        );
    }

    #[test]
    fn partially_filled_tank_limits_first_trip_only() {
        // Tank 100 but level 10: a 90-demand customer needs a refill first.
        let base = problem_small_tank_with_refill();
        let vehicle = Vehicle {
            tank: Tank::new(Liters::new(100.0).unwrap(), Liters::new(10.0).unwrap()).unwrap(),
            ..base.vehicles()[0]
        };
        let mut customers = base.customers().to_vec();
        customers[0] = Customer {
            demand: Liters::new(90.0).unwrap(),
            ..customers[0]
        };
        let problem = Problem::new(
            vec![vehicle],
            base.depots().to_vec(),
            customers,
            base.refill_stations().to_vec(),
        )
        .unwrap();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        assert!(!instance.evaluate(0, &[2]).is_feasible, "90 > level 10");
        assert!(
            instance.evaluate(0, &[4, 2]).is_feasible,
            "refill first, then serve 90"
        );
    }

    #[test]
    fn stop_maps_ids_back_to_domain() {
        let problem = problem_small_tank_with_refill();
        let instance = Instance::new(&problem, &uniform_matrix(5)).unwrap();
        assert!(matches!(instance.stop(0), Stop::VehicleStart(id) if id.get() == 1));
        assert!(matches!(instance.stop(1), Stop::Depot(id) if id.get() == 1));
        assert!(matches!(instance.stop(2), Stop::Customer(id) if id.get() == 1));
        assert!(matches!(instance.stop(3), Stop::Customer(id) if id.get() == 2));
        assert!(matches!(instance.stop(4), Stop::Refill(id) if id.get() == 1));
    }
}
