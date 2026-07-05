/// Load segment after PyVRP / Vidal et al. (2014) eqs. (9)-(11).
/// Enables O(1) capacity checks when concatenating route pieces.
/// Concatenation (`merge`) follows Vidal et al.; `finalize` is a
/// project-specific extension modeling reload/trip-boundary resets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadSegment {
    /// Total demand delivered within this segment.
    pub(crate) delivery: f64,
    /// Running load at the end of the segment.
    pub(crate) load: f64,
    /// Capacity excess accumulated in already-finalized trips.
    pub(crate) excess: f64,
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
        // Merging demands never loses total delivery.
        fn merge_preserves_delivery(a: f64, b: f64) -> bool {
            if !a.is_finite() || !b.is_finite() { return true; }
            let (a, b) = (a.abs() % 1e6, b.abs() % 1e6);
            let s = LoadSegment::merge(LoadSegment::from_customer(a), LoadSegment::from_customer(b));
            (s.delivery - (a + b)).abs() < 1e-6
        }
    }

    #[test]
    fn feasibility_tolerates_float_noise() {
        let s = LoadSegment::from_customer(CAP + 1e-10);
        assert!(s.is_feasible(CAP));
        let s = LoadSegment::from_customer(CAP + 1e-8);
        assert!(!s.is_feasible(CAP));
    }

    #[test]
    fn empty_is_identity_for_merge() {
        let x = LoadSegment::from_customer(42.0);
        assert_eq!(LoadSegment::merge(LoadSegment::EMPTY, x), x);
        assert_eq!(LoadSegment::merge(x, LoadSegment::EMPTY), x);
    }

    quickcheck! {
        // merge is associative on all fields for customer-built segments.
        fn merge_is_associative(a: f64, b: f64, c: f64) -> bool {
            if !a.is_finite() || !b.is_finite() || !c.is_finite() { return true; }
            let seg = |v: f64| LoadSegment::from_customer(v.abs() % 1e6);
            let (a, b, c) = (seg(a), seg(b), seg(c));
            let left = LoadSegment::merge(LoadSegment::merge(a, b), c);
            let right = LoadSegment::merge(a, LoadSegment::merge(b, c));
            (left.delivery - right.delivery).abs() < 1e-6
                && (left.load - right.load).abs() < 1e-6
                && (left.excess - right.excess).abs() < 1e-6
        }
    }
}
