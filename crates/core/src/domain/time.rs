use serde::{Deserialize, Serialize};

use crate::domain::{DomainError, Duration};
use crate::non_negative_unit;

non_negative_unit!(Time);

impl std::ops::Add<Duration> for Time {
    type Output = Time;

    fn add(self, rhs: Duration) -> Self::Output {
        Time(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Time> for Time {
    type Output = Duration;

    fn sub(self, rhs: Time) -> Self::Output {
        Duration((self.0 - rhs.0).max(0.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TimeWindowRaw")]
pub struct TimeWindow {
    start: Time,
    end: Time,
}

#[derive(Deserialize)]
struct TimeWindowRaw {
    start: Time,
    end: Time,
}

impl TimeWindow {
    pub fn new(start: Time, end: Time) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::TimeWindowOrder {
                start: start.get(),
                end: end.get(),
            });
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> Time {
        self.start
    }

    pub const fn end(self) -> Time {
        self.end
    }

    pub fn contains(self, t: Time) -> bool {
        self.start <= t && t <= self.end
    }
}

impl TryFrom<TimeWindowRaw> for TimeWindow {
    type Error = DomainError;

    fn try_from(value: TimeWindowRaw) -> Result<Self, Self::Error> {
        Self::new(value.start, value.end)
    }
}
