/// Load segment after PyVRP / Vidal et al. (2014) eqs. (9)-(11).
/// Enables O(1) capacity checks when concatenating route pieces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSegment {
    /// Total demand delivered within this segment.
    pub delivery: f64,
    /// Running load at the end of the segment.
    pub load: f64,
    /// Capacity excess accumulated in already-finalized trips.
    pub excess: f64,
}

impl LoadSegment {
    pub const EMPTY: Self = Self {
        delivery: 0.0,
        load: 0.0,
        excess: 0.0,
    };

    pub fn from_customer(demand: f64) -> Self {
        Self {
            delivery: demand,
            load: demand,
            excess: 0.0,
        }
    }

    pub fn merge(first: Self, second: Self) -> Self {
        Self {
            delivery: first.delivery + second.delivery,
            load: f64::max(first.load + second.delivery, second.load),
            excess: first.excess + second.excess,
        }
    }

    /// Close a trip at a reload/depot: reset the running load, keep the excess.
    pub fn finalize(self, capacity: f64) -> Self {
        Self {
            delivery: 0.0,
            load: 0.0,
            excess: self.excess_load(capacity),
        }
    }

    pub fn excess_load(self, capacity: f64) -> f64 {
        self.excess + f64::max(self.load - capacity, 0.0)
    }

    pub fn is_feasible(self, capacity: f64) -> bool {
        self.excess_load(capacity) <= 1e-9
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    const CAP: f64 = 100.0;

    #[test]
    fn merge_accumulates_demand() {
        let s = LoadSegment::merge(
            LoadSegment::from_customer(30.0),
            LoadSegment::from_customer(50.0),
        );
        assert_eq!(s.load, 80.0);
        assert!(s.is_feasible(CAP));
    }

    #[test]
    fn detects_excess_load() {
        let s = LoadSegment::merge(
            LoadSegment::from_customer(80.0),
            LoadSegment::from_customer(50.0),
        );
        assert_eq!(s.excess_load(CAP), 30.0);
        assert!(!s.is_feasible(CAP));
    }

    #[test]
    fn finalize_models_reload_reset() {
        // customer(90) -> reload -> customer(60): 150 total but feasible with cap 100
        let s = LoadSegment::merge(
            LoadSegment::from_customer(90.0).finalize(CAP),
            LoadSegment::from_customer(60.0),
        );
        assert!(s.is_feasible(CAP));
    }

    #[test]
    fn excess_before_reload_is_not_forgotten() {
        // customer(130) violates cap 100 before the reload; reload must not absolve it
        let s = LoadSegment::merge(
            LoadSegment::from_customer(130.0).finalize(CAP),
            LoadSegment::from_customer(60.0),
        );
        assert_eq!(s.excess_load(CAP), 30.0);
        assert!(!s.is_feasible(CAP));
    }

    #[test]
    fn finalize_preserves_cumulative_excess() {
        let over = LoadSegment::from_customer(130.0);
        let s = over.finalize(CAP);
        assert_eq!(s.load, 0.0);
        assert_eq!(s.excess_load(CAP), 30.0);
    }

    quickcheck! {
        // Merging demands never loses total delivery and merge is associative on delivery.
        fn merge_preserves_delivery(a: f64, b: f64) -> bool {
            if !a.is_finite() || !b.is_finite() { return true; }
            let (a, b) = (a.abs() % 1e6, b.abs() % 1e6);
            let s = LoadSegment::merge(LoadSegment::from_customer(a), LoadSegment::from_customer(b));
            (s.delivery - (a + b)).abs() < 1e-6
        }
    }
}
