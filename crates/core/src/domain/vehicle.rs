use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use crate::domain::{Coordinate, DomainError, Id, Kilograms, Liters, Meters, TimeWindow};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TankRaw")]
pub struct Tank {
    capacity: Liters,
    level: Liters,
}

#[derive(Deserialize)]
struct TankRaw {
    capacity: Liters,
    level: Liters,
}

impl Tank {
    pub fn new(capacity: Liters, level: Liters) -> Result<Self, DomainError> {
        if level.get() > capacity.get() {
            return Err(DomainError::Overfilled {
                level: level.get(),
                capacity: capacity.get(),
            });
        }
        Ok(Self { capacity, level })
    }

    pub fn full(capacity: Liters) -> Self {
        Self {
            capacity,
            level: capacity,
        }
    }

    pub const fn capacity(self) -> Liters {
        self.capacity
    }

    pub const fn level(self) -> Liters {
        self.level
    }

    pub fn remaining(self) -> Liters {
        Liters::new(self.capacity.get() - self.level.get())
            .expect("capacity >= level holds by construction")
    }

    pub fn can_serve(self, demand: Liters) -> bool {
        demand.get() <= self.level.get()
    }
}

impl TryFrom<TankRaw> for Tank {
    type Error = DomainError;

    fn try_from(value: TankRaw) -> Result<Self, Self::Error> {
        Self::new(value.capacity, value.level)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VehicleKind {
    Car {
        width: Meters,
        height: Meters,
    },
    Truck {
        width: Meters,
        height: Meters,
        length: Meters,
        weight: Kilograms,
    },
}

pub type VehicleId = Id<Vehicle>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: VehicleId,
    pub start: Coordinate,
    pub tank: Tank,
    pub kind: VehicleKind,
    pub shift: TimeWindow,
    pub max_trips: Option<NonZeroU32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_rejects_overfilled_tank() {
        assert!(serde_json::from_str::<Tank>(r#"{"capacity":100.0,"level":200.0}"#).is_err());
        assert!(serde_json::from_str::<Tank>(r#"{"capacity":100.0,"level":50.0}"#).is_ok());
    }
}
