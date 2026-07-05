use serde::{Deserialize, Serialize};

use crate::domain::{Coordinate, Duration, Id, Liters, TimeWindow};

pub type CustomerId = Id<Customer>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Customer {
    pub id: CustomerId,
    pub location: Coordinate,
    pub demand: Liters,
    pub service_time: Duration,
    pub time_window: Option<TimeWindow>,
}
