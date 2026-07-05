use serde::{Deserialize, Serialize};

use crate::domain::{CustomerId, Duration, Meters, Stop, VehicleId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub vehicle: VehicleId,
    pub stops: Vec<Stop>,
    pub distance: Meters,
    pub travel_time: Duration,
    pub wait_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub routes: Vec<Route>,
    pub unserved: Vec<CustomerId>,
    pub total_distance: Meters,
    pub total_travel_time: Duration,
}
