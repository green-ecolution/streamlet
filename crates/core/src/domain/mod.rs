use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub mod coordinate;
pub mod customer;
pub mod node;
pub mod problem;
pub mod solution;
pub mod time;
pub mod vehicle;

pub use coordinate::Coordinate;
pub use customer::{Customer, CustomerId};
pub use node::{Depot, DepotId, RefillStation, RefillStationId};
pub use problem::{Problem, Stop};
pub use solution::{Route, Solution};
pub use time::{Time, TimeWindow};
pub use vehicle::{Tank, Vehicle, VehicleId, VehicleKind};

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("value must be finite and non-negative, got {0}")]
    NonNegative(f64),
    #[error("latitude {0} out of range [-90, 90]")]
    Latitude(f64),
    #[error("longitude {0} out of range [-180, 180]")]
    Longitude(f64),
    #[error("time window end {end} before start {start}")]
    TimeWindowOrder { start: f64, end: f64 },
    #[error("load {level} exceeds capacity {capacity}")]
    Overfilled { level: f64, capacity: f64 },
    #[error("problem needs at least one {0}")]
    Empty(&'static str),
}

type RawId = u32;

pub struct Id<T> {
    value: RawId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub const fn new(value: RawId) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    pub const fn get(self) -> RawId {
        self.value
    }
}

// Hand-written so Id<T> demands nothing of the phantom marker T (a derive would
// add spurious T: Clone/Debug/Serialize bounds) and serializes as a bare id
// instead of a struct carrying the _marker field.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, o: &Self) -> bool {
        self.value == o.value
    }
}
impl<T> Eq for Id<T> {}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        self.value.hash(s)
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.value)
    }
}
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}
impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        RawId::deserialize(deserializer).map(Self::new)
    }
}

// Self-contained: uses ::serde and $crate so callers need no matching `use`.
#[macro_export]
macro_rules! non_negative_unit {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, PartialOrd, ::serde::Serialize, ::serde::Deserialize,
        )]
        #[serde(try_from = "f64", into = "f64")]
        pub struct $name(f64);

        impl $name {
            pub fn new(value: f64) -> Result<Self, $crate::domain::DomainError> {
                if value.is_finite() && value >= 0.0 {
                    Ok(Self(value))
                } else {
                    Err($crate::domain::DomainError::NonNegative(value))
                }
            }

            pub const fn get(self) -> f64 {
                self.0
            }
        }

        impl TryFrom<f64> for $name {
            type Error = $crate::domain::DomainError;
            fn try_from(v: f64) -> Result<Self, Self::Error> {
                Self::new(v)
            }
        }

        impl From<$name> for f64 {
            fn from(v: $name) -> f64 {
                v.0
            }
        }
    };
}

non_negative_unit!(Liters);
non_negative_unit!(Meters);
non_negative_unit!(Kilograms);
non_negative_unit!(Duration);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_uses_bare_number_wire_format() {
        let id: Id<()> = Id::new(42);
        assert_eq!(serde_json::to_string(&id).unwrap(), "42");

        let back: Id<()> = serde_json::from_str("42").unwrap();
        assert_eq!(back.get(), 42);
    }

    #[test]
    fn non_negative_unit_validates_on_deserialize() {
        assert!(serde_json::from_str::<Liters>("-1.0").is_err());
        assert!(serde_json::from_str::<Liters>("3.5").is_ok());
    }
}
