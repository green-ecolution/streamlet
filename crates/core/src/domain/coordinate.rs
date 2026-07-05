use serde::{Deserialize, Serialize};

use crate::domain::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CoordinateRaw")]
pub struct Coordinate {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct CoordinateRaw {
    lat: f64,
    lon: f64,
}

impl Coordinate {
    pub fn new(lat: f64, lon: f64) -> Result<Self, DomainError> {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(DomainError::Latitude(lat));
        }
        if !(-180.0..=180.0).contains(&lon) {
            return Err(DomainError::Longitude(lon));
        }
        Ok(Self { lat, lon })
    }

    pub const fn lat(self) -> f64 {
        self.lat
    }

    pub const fn lon(self) -> f64 {
        self.lon
    }
}

impl TryFrom<CoordinateRaw> for Coordinate {
    type Error = DomainError;

    fn try_from(value: CoordinateRaw) -> Result<Self, Self::Error> {
        Self::new(value.lat, value.lon)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_validates_range() {
        assert!(serde_json::from_str::<Coordinate>(r#"{"lat":91.0,"lon":0.0}"#).is_err());
        assert!(serde_json::from_str::<Coordinate>(r#"{"lat":54.78,"lon":9.44}"#).is_ok());
    }
}
