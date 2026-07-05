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
