use serde::{Deserialize, Serialize};

/// Index into a [`CostMatrix`]; assigned by node order in a `Problem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeIndex(pub usize);

#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error(
        "matrix must be square and equally sized, got {time}x? time and {distance}x? distance rows"
    )]
    Shape { time: usize, distance: usize },
    #[error("matrix values must be finite and non-negative")]
    Value,
}

/// Square travel-time (seconds) and distance (meters) matrix over problem nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostMatrix {
    travel_time: Vec<Vec<f64>>,
    distance: Vec<Vec<f64>>,
}

impl CostMatrix {
    pub fn new(travel_time: Vec<Vec<f64>>, distance: Vec<Vec<f64>>) -> Result<Self, MatrixError> {
        let n = travel_time.len();
        let square = |m: &[Vec<f64>]| m.iter().all(|row| row.len() == m.len());
        if distance.len() != n || !square(&travel_time) || !square(&distance) {
            return Err(MatrixError::Shape {
                time: n,
                distance: distance.len(),
            });
        }
        let valid = |m: &[Vec<f64>]| m.iter().flatten().all(|v| v.is_finite() && *v >= 0.0);
        if !valid(&travel_time) || !valid(&distance) {
            return Err(MatrixError::Value);
        }
        Ok(Self {
            travel_time,
            distance,
        })
    }

    pub fn len(&self) -> usize {
        self.travel_time.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn travel_time(&self, from: NodeIndex, to: NodeIndex) -> f64 {
        self.travel_time[from.0][to.0]
    }

    pub fn distance(&self, from: NodeIndex, to: NodeIndex) -> f64 {
        self.distance[from.0][to.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix_2x2() -> CostMatrix {
        CostMatrix::new(
            vec![vec![0.0, 10.0], vec![12.0, 0.0]],
            vec![vec![0.0, 100.0], vec![120.0, 0.0]],
        )
        .unwrap()
    }

    #[test]
    fn returns_costs_by_index() {
        let m = matrix_2x2();
        assert_eq!(m.travel_time(NodeIndex(0), NodeIndex(1)), 10.0);
        assert_eq!(m.distance(NodeIndex(1), NodeIndex(0)), 120.0);
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn rejects_non_square_or_mismatched_input() {
        assert!(CostMatrix::new(vec![vec![0.0, 1.0]], vec![vec![0.0]]).is_err());
        assert!(CostMatrix::new(vec![vec![0.0], vec![1.0]], vec![vec![0.0], vec![1.0]]).is_err());
        assert!(
            CostMatrix::new(
                vec![vec![0.0, f64::NAN], vec![1.0, 0.0]],
                vec![vec![0.0, 1.0], vec![1.0, 0.0]]
            )
            .is_err()
        );
    }
}
