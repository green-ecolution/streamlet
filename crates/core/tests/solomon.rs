use streamlet_core::domain::*;
use streamlet_core::matrix::CostMatrix;
use streamlet_core::solver::{SolveOptions, solve};

struct SolomonInstance {
    capacity: f64,
    n_vehicles: u32,
    /// (x, y, demand, ready, due, service); index 0 is the depot.
    rows: Vec<(f64, f64, f64, f64, f64, f64)>,
}

fn parse(name: &str) -> SolomonInstance {
    let text = std::fs::read_to_string(format!("tests/fixtures/solomon/{name}.txt")).unwrap();
    let mut lines = text.lines();
    let mut capacity = 0.0;
    let mut n_vehicles = 0;
    let mut rows = vec![];
    while let Some(line) = lines.next() {
        if line.trim() == "VEHICLE" {
            lines.next(); // header
            let nums: Vec<f64> = lines
                .next()
                .unwrap()
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect();
            n_vehicles = nums[0] as u32;
            capacity = nums[1];
        }
        let fields: Vec<f64> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if fields.len() == 7 {
            rows.push((
                fields[1], fields[2], fields[3], fields[4], fields[5], fields[6],
            ));
        }
    }
    SolomonInstance {
        capacity,
        n_vehicles,
        rows,
    }
}

fn build(instance: &SolomonInstance) -> (Problem, CostMatrix) {
    let depot_row = instance.rows[0];
    // Solomon coordinates are planar; scale into valid lat/lon ranges (values
    // are only carried through, the matrix below uses the original coords).
    let coord = |x: f64, y: f64| Coordinate::new(x / 10.0, y / 10.0).unwrap();
    let vehicles: Vec<Vehicle> = (0..instance.n_vehicles)
        .map(|i| Vehicle {
            id: VehicleId::new(i + 1),
            start: coord(depot_row.0, depot_row.1),
            tank: Tank::full(Liters::new(instance.capacity).unwrap()),
            kind: VehicleKind::Car {
                width: Meters::new(2.0).unwrap(),
                height: Meters::new(2.0).unwrap(),
            },
            shift: TimeWindow::new(
                Time::new(depot_row.3).unwrap(),
                Time::new(depot_row.4).unwrap(),
            )
            .unwrap(),
            max_trips: None,
        })
        .collect();
    let depot = Depot {
        id: DepotId::new(1),
        location: coord(depot_row.0, depot_row.1),
    };
    let customers: Vec<Customer> = instance.rows[1..]
        .iter()
        .enumerate()
        .map(|(i, r)| Customer {
            id: CustomerId::new(i as u32 + 1),
            location: coord(r.0, r.1),
            demand: Liters::new(r.2).unwrap(),
            service_time: Duration::new(r.5).unwrap(),
            time_window: Some(
                TimeWindow::new(Time::new(r.3).unwrap(), Time::new(r.4).unwrap()).unwrap(),
            ),
        })
        .collect();
    let problem = Problem::new(vehicles, vec![depot], customers, vec![]).unwrap();

    // Matrix over [vehicles | depot | customers]: Euclidean on original coords.
    let mut points: Vec<(f64, f64)> =
        vec![(depot_row.0, depot_row.1); instance.n_vehicles as usize];
    points.push((depot_row.0, depot_row.1));
    points.extend(instance.rows[1..].iter().map(|r| (r.0, r.1)));
    let dist: Vec<Vec<f64>> = points
        .iter()
        .map(|a| {
            points
                .iter()
                .map(|b| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt())
                .collect()
        })
        .collect();
    let matrix = CostMatrix::new(dist.clone(), dist).unwrap();
    (problem, matrix)
}

/// Best-known distances (Solomon 100-customer): c101 828.94, r101 1650.80, rc101 1696.95.
fn assert_instance(name: &str, bks: f64, max_gap: f64) {
    let (problem, matrix) = build(&parse(name));
    let solution = solve(&problem, &matrix, &SolveOptions::default()).unwrap();
    assert!(
        solution.unserved.is_empty(),
        "{name}: unserved {:?}",
        solution.unserved
    );
    let total = solution.total_distance.get();
    let gap = (total - bks) / bks;
    println!(
        "{name}: distance {total:.2}, BKS {bks:.2}, gap {:.1}%",
        gap * 100.0
    );
    assert!(
        gap <= max_gap,
        "{name}: distance {total:.2}, BKS {bks:.2}, gap {:.1}% > {:.0}%",
        gap * 100.0,
        max_gap * 100.0
    );
}

// Measured baseline (2026-07-05, default SolveOptions, release build):
// c101 854.31 (gap 3.1%), r101 1682.58 (gap 1.9%), rc101 1722.32 (gap 1.5%).
// The 5% bound is measured-plus-headroom; tighten it as the solver improves.

#[test]
fn c101_within_gap() {
    assert_instance("c101", 828.94, 0.05)
}

#[test]
fn r101_within_gap() {
    assert_instance("r101", 1650.80, 0.05)
}

#[test]
fn rc101_within_gap() {
    assert_instance("rc101", 1696.95, 0.05)
}
