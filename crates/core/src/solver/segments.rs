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

/// Duration segment after PyVRP: tracks duration, time warp, and the
/// earliest/latest feasible start of the segment. Multi-trip state lives in
/// `cum_*` and `prev_end_late` and is folded in by `finalise_back`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DurationSegment {
    /// Raw accumulated duration of this segment (travel + service + wait).
    /// Distinct from the [`duration()`](Self::duration) method, which adds
    /// `cum_duration` and any forced inter-trip wait on top of this field.
    pub(crate) duration: f64,
    /// Late-arrival penalty accumulated within this segment.
    pub(crate) time_warp: f64,
    /// Earliest feasible start time of this segment.
    pub(crate) start_early: f64,
    /// Latest feasible start time of this segment.
    pub(crate) start_late: f64,
    /// Earliest departure constraint used for multi-trip coordination.
    pub(crate) release_time: f64,
    /// Folded-in duration total of already-finalized trips.
    pub(crate) cum_duration: f64,
    /// Folded-in time warp total of already-finalized trips.
    pub(crate) cum_time_warp: f64,
    /// Latest end time of the previous trip.
    pub(crate) prev_end_late: f64,
}

impl DurationSegment {
    pub const EMPTY: Self = Self {
        duration: 0.0,
        time_warp: 0.0,
        start_early: 0.0,
        start_late: f64::INFINITY,
        release_time: 0.0,
        cum_duration: 0.0,
        cum_time_warp: 0.0,
        prev_end_late: f64::INFINITY,
    };

    pub fn from_node(tw_early: f64, tw_late: f64, service_time: f64) -> Self {
        Self {
            duration: service_time,
            start_early: tw_early,
            start_late: tw_late,
            ..Self::EMPTY
        }
    }

    pub fn merge(first: Self, second: Self, edge_duration: f64) -> Self {
        let at_second = first.duration - first.time_warp + edge_duration;
        let arrival = first.start_early + at_second;
        let diff_tw = f64::max(arrival - second.start_late, 0.0);
        let diff_wait = f64::max(second.start_early - at_second - first.start_late, 0.0);
        // Guard against large negative start_late - at_second (PyVRP overflow check).
        let second_late = if at_second > second.start_late + 1000.0 {
            second.start_late
        } else {
            second.start_late - at_second
        };
        Self {
            duration: first.duration + second.duration + edge_duration + diff_wait,
            time_warp: first.time_warp + second.time_warp + diff_tw,
            start_early: f64::max(first.start_early, second.start_early - at_second) - diff_wait,
            start_late: f64::min(first.start_late, second_late) + diff_tw,
            release_time: f64::max(first.release_time, second.release_time),
            cum_duration: first.cum_duration + second.cum_duration,
            cum_time_warp: first.cum_time_warp + second.cum_time_warp,
            prev_end_late: first.prev_end_late,
        }
    }

    fn finalise_front(&self) -> Self {
        let curr = Self {
            release_time: 0.0,
            cum_duration: 0.0,
            cum_time_warp: 0.0,
            prev_end_late: f64::INFINITY,
            ..*self
        };
        let release = Self {
            start_early: self.start_early(),
            start_late: self.start_late(),
            ..Self::EMPTY
        };
        Self::merge(release, curr, 0.0)
    }

    /// Close a trip at a reload: the returned segment is the start of the next
    /// trip, which cannot begin before this trip's earliest end.
    pub fn finalise_back(&self) -> Self {
        let prev = Self {
            start_late: self.prev_end_late,
            ..Self::EMPTY
        };
        let finalised = Self::merge(prev, self.finalise_front(), 0.0);
        Self {
            start_early: finalised.end_early(),
            release_time: finalised.end_early(),
            cum_duration: self.cum_duration + finalised.duration(),
            cum_time_warp: self.cum_time_warp + finalised.time_warp_total(),
            prev_end_late: finalised.end_late(),
            ..Self::EMPTY
        }
    }

    /// Earliest end time of this trip, separating trip-specific values from
    /// the folded-in cumulative totals of previously finalized trips.
    pub fn end_early(&self) -> f64 {
        let trip_duration = self.duration() - self.cum_duration;
        let trip_tw = self.time_warp_total() - self.cum_time_warp;
        self.start_early() + trip_duration - trip_tw
    }

    /// Latest end time of this trip, separating trip-specific values from
    /// the folded-in cumulative totals of previously finalized trips.
    pub fn end_late(&self) -> f64 {
        let trip_duration = self.duration() - self.cum_duration;
        let trip_tw = self.time_warp_total() - self.cum_time_warp;
        let net = trip_duration - trip_tw;
        if net > f64::MAX - self.start_late() {
            f64::MAX
        } else {
            self.start_late() + net
        }
    }

    /// Total effective duration: the minimal duration over all feasible start
    /// times, including already-finalized trips (`cum_duration`) plus any
    /// forced depot wait when this trip's earliest start would otherwise
    /// precede the previous trip's earliest possible end (`prev_end_late`).
    /// Uses the [`start_early()`](Self::start_early) accessor rather than the
    /// raw field so that `release_time` is accounted for.
    pub fn duration(&self) -> f64 {
        self.cum_duration + self.duration + f64::max(self.start_early() - self.prev_end_late, 0.0)
    }

    /// Effective earliest start time, clamped by `release_time`.
    pub fn start_early(&self) -> f64 {
        f64::max(self.start_early, self.release_time)
    }

    /// Effective latest start time, clamped by `release_time`.
    pub fn start_late(&self) -> f64 {
        f64::max(self.start_late, self.release_time)
    }

    pub fn time_warp_total(&self) -> f64 {
        self.cum_time_warp + self.time_warp
    }

    /// Time warp including shift-length (`max_duration`) and release violations.
    pub fn time_warp_with_max(&self, max_duration: f64) -> f64 {
        let tw = self.time_warp_total();
        let net = self.duration() - tw;
        // Deliberately uses the raw `start_late` field, not the `start_late()`
        // accessor: the accessor clamps by `release_time`, which would make
        // `release_time - start_late` structurally <= 0 and hide the violation.
        tw + f64::max(self.release_time - self.start_late, 0.0) + f64::max(net - max_duration, 0.0)
    }

    pub fn is_feasible(&self) -> bool {
        self.time_warp_total() <= 1e-9
    }

    pub fn is_feasible_with_max(&self, max_duration: f64) -> bool {
        self.time_warp_with_max(max_duration) <= 1e-9
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

    fn node(early: f64, late: f64, service: f64) -> DurationSegment {
        DurationSegment::from_node(early, late, service)
    }

    #[test]
    fn sequential_visits_within_windows_are_feasible() {
        // depot [0,1000] -> customer [100,200] svc 10, travel 50
        let s = DurationSegment::merge(node(0.0, 1000.0, 0.0), node(100.0, 200.0, 10.0), 50.0);
        assert!(s.is_feasible());
        // duration() is the MINIMAL duration over feasible start times: the wide
        // depot window lets the vehicle leave at 50 and arrive exactly at the
        // customer opening (100) with zero wait -> 50 travel + 10 service.
        assert_eq!(s.duration(), 60.0);
        assert_eq!(s.start_early(), 50.0);
        assert_eq!(s.end_early(), 110.0);
    }

    #[test]
    fn narrow_first_window_forces_waiting() {
        // depot must start in [0,0] -> arrival at 50 < opening 100 -> 50 wait
        let s = DurationSegment::merge(node(0.0, 0.0, 0.0), node(100.0, 200.0, 10.0), 50.0);
        assert!(s.is_feasible());
        assert_eq!(s.duration(), 110.0); // 50 travel + 50 wait + 10 service
    }

    #[test]
    fn late_arrival_creates_time_warp() {
        // window closes at 40 but travel alone takes 50
        let s = DurationSegment::merge(node(0.0, 0.0, 0.0), node(0.0, 40.0, 0.0), 50.0);
        assert!(!s.is_feasible());
        assert!(s.time_warp_total() > 0.0);
    }

    #[test]
    fn merge_is_order_dependent_but_associative_in_feasibility() {
        let a = node(0.0, 100.0, 5.0);
        let b = node(50.0, 60.0, 5.0);
        let c = node(200.0, 210.0, 5.0);
        let left = DurationSegment::merge(DurationSegment::merge(a, b, 10.0), c, 10.0);
        let right = DurationSegment::merge(a, DurationSegment::merge(b, c, 10.0), 10.0);
        assert_eq!(left.is_feasible(), right.is_feasible());
        assert!((left.time_warp_total() - right.time_warp_total()).abs() < 1e-9);
    }

    #[test]
    fn finalise_back_starts_next_trip_after_previous_ends() {
        let trip1 = DurationSegment::merge(node(0.0, 10.0, 0.0), node(0.0, 1000.0, 30.0), 20.0);
        let next = trip1.finalise_back();
        // Next trip cannot start before trip1's earliest end (0 + 20 + 30 = 50).
        assert_eq!(next.start_early(), 50.0);
        assert_eq!(next.time_warp_total(), 0.0);
    }

    #[test]
    fn shift_end_violation_shows_as_time_warp_with_max() {
        let s = DurationSegment::merge(node(0.0, 0.0, 0.0), node(0.0, 1000.0, 100.0), 100.0);
        assert!(s.is_feasible());
        assert!(s.time_warp_with_max(150.0) > 0.0); // 200 net duration > 150 shift
        assert_eq!(s.time_warp_with_max(300.0), 0.0);
    }

    #[test]
    fn duration_empty_is_identity_for_merge() {
        let x = node(50.0, 60.0, 5.0);
        assert_eq!(DurationSegment::merge(DurationSegment::EMPTY, x, 0.0), x);
        assert_eq!(DurationSegment::merge(x, DurationSegment::EMPTY, 0.0), x);
    }

    #[test]
    fn two_reloads_accumulate_across_three_trips() {
        let trip = |seg: DurationSegment| {
            DurationSegment::merge(seg, node(0.0, f64::INFINITY, 30.0), 20.0)
        };
        let t1 = trip(node(0.0, 0.0, 0.0));
        let t2 = trip(t1.finalise_back());
        let t3 = trip(t2.finalise_back());
        // Each trip: 20 travel + 30 service = 50; three trips = 150 total.
        assert_eq!(t3.duration(), 150.0);
        assert!(t3.is_feasible());
        // Third trip cannot start before the second ends (2 x 50).
        assert_eq!(t3.start_early(), 100.0);
    }
}
