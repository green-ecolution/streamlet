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

#[cfg(test)]
mod tests {
    use super::*;

    fn time(v: f64) -> Time {
        Time::new(v).unwrap()
    }

    fn dur(v: f64) -> Duration {
        Duration::new(v).unwrap()
    }

    #[test]
    fn add_duration_advances_time() {
        assert_eq!((time(10.0) + dur(5.0)).get(), 15.0);
    }

    #[test]
    fn sub_time_saturates_at_zero() {
        assert_eq!((time(10.0) - time(4.0)).get(), 6.0);
        assert_eq!((time(4.0) - time(10.0)).get(), 0.0);
    }

    #[test]
    fn window_rejects_end_before_start() {
        assert!(TimeWindow::new(time(10.0), time(5.0)).is_err());
    }

    #[test]
    fn window_allows_zero_width() {
        assert!(TimeWindow::new(time(5.0), time(5.0)).is_ok());
    }

    #[test]
    fn contains_is_inclusive() {
        let window = TimeWindow::new(time(5.0), time(10.0)).unwrap();
        assert!(window.contains(time(5.0)));
        assert!(window.contains(time(10.0)));
        assert!(window.contains(time(7.0)));
        assert!(!window.contains(time(4.9)));
        assert!(!window.contains(time(10.1)));
    }
}
