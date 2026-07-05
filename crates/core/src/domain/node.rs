use serde::{Deserialize, Serialize};

use crate::domain::{Coordinate, Duration, Id};

pub type DepotId = Id<Depot>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Depot {
    pub id: DepotId,
    pub location: Coordinate,
}

pub type RefillStationId = Id<RefillStation>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RefillStation {
    pub id: RefillStationId,
    pub location: Coordinate,
    pub refill_duration: Duration,
}
