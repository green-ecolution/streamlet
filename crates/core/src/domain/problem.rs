use serde::{Deserialize, Serialize};

use crate::domain::{
    Customer, CustomerId, Depot, DepotId, DomainError, RefillStation, RefillStationId, Vehicle,
    VehicleId,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Stop {
    VehicleStart(VehicleId),
    Customer(CustomerId),
    Depot(DepotId),
    Refill(RefillStationId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "ProblemRaw")]
pub struct Problem {
    vehicles: Vec<Vehicle>,
    depots: Vec<Depot>,
    customers: Vec<Customer>,
    refill_stations: Vec<RefillStation>,
}

impl Problem {
    pub fn new(
        vehicles: Vec<Vehicle>,
        depots: Vec<Depot>,
        customers: Vec<Customer>,
        refill_stations: Vec<RefillStation>,
    ) -> Result<Self, DomainError> {
        if vehicles.is_empty() {
            return Err(DomainError::Empty("vehicle"));
        }
        if depots.is_empty() {
            return Err(DomainError::Empty("depot"));
        }
        Ok(Self {
            vehicles,
            depots,
            customers,
            refill_stations,
        })
    }

    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    pub fn depots(&self) -> &[Depot] {
        &self.depots
    }

    pub fn customers(&self) -> &[Customer] {
        &self.customers
    }

    pub fn refill_stations(&self) -> &[RefillStation] {
        &self.refill_stations
    }
}

#[derive(Deserialize)]
struct ProblemRaw {
    vehicles: Vec<Vehicle>,
    depots: Vec<Depot>,
    customers: Vec<Customer>,
    refill_stations: Vec<RefillStation>,
}

impl TryFrom<ProblemRaw> for Problem {
    type Error = DomainError;

    fn try_from(value: ProblemRaw) -> Result<Self, Self::Error> {
        Self::new(
            value.vehicles,
            value.depots,
            value.customers,
            value.refill_stations,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Coordinate, Liters, Meters, Tank, Time, TimeWindow, VehicleKind};

    fn depot() -> Depot {
        Depot {
            id: DepotId::new(1),
            location: Coordinate::new(54.0, 9.0).unwrap(),
        }
    }

    fn vehicle() -> Vehicle {
        Vehicle {
            id: VehicleId::new(1),
            start: Coordinate::new(54.0, 9.0).unwrap(),
            tank: Tank::full(Liters::new(100.0).unwrap()),
            kind: VehicleKind::Car {
                width: Meters::new(2.0).unwrap(),
                height: Meters::new(2.0).unwrap(),
            },
            shift: TimeWindow::new(Time::new(0.0).unwrap(), Time::new(100.0).unwrap()).unwrap(),
            max_trips: None,
        }
    }

    #[test]
    fn requires_at_least_one_vehicle() {
        assert!(Problem::new(vec![], vec![depot()], vec![], vec![]).is_err());
    }

    #[test]
    fn requires_at_least_one_depot() {
        assert!(Problem::new(vec![vehicle()], vec![], vec![], vec![]).is_err());
    }

    #[test]
    fn accepts_minimal_problem() {
        assert!(Problem::new(vec![vehicle()], vec![depot()], vec![], vec![]).is_ok());
    }
}
