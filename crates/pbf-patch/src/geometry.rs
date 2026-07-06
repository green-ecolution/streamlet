use crate::model::Coordinate;

const EARTH_RADIUS_M: f64 = 6_371_000.0;

pub fn haversine_distance(a: Coordinate, b: Coordinate) -> f64 {
    let d_lat = (b.lat - a.lat).to_radians();
    let d_lon = (b.lon - a.lon).to_radians();

    let h = (d_lat / 2.0).sin().powi(2)
        + a.lat.to_radians().cos() * b.lat.to_radians().cos() * (d_lon / 2.0).sin().powi(2);

    2.0 * EARTH_RADIUS_M * h.sqrt().atan2((1.0 - h).sqrt())
}

pub fn point_to_segment_distance(point: Coordinate, seg1: Coordinate, seg2: Coordinate) -> f64 {
    let (px, py) = (point.lon, point.lat);
    let (ax, ay) = (seg1.lon, seg1.lat);
    let (bx, by) = (seg2.lon, seg2.lat);

    let (abx, aby) = (bx - ax, by - ay);
    let (apx, apy) = (px - ax, py - ay);

    let ab2 = abx * abx + aby * aby;
    if ab2 == 0.0 {
        return haversine_distance(point, seg1);
    }

    let t = ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0);
    let closest = Coordinate {
        lat: ay + t * aby,
        lon: ax + t * abx,
    };

    haversine_distance(point, closest)
}

pub fn point_to_line_distance(point: Coordinate, line: &[Coordinate]) -> f64 {
    match line {
        [] => f64::MAX,
        [single] => haversine_distance(point, *single),
        _ => line
            .windows(2)
            .map(|seg| point_to_segment_distance(point, seg[0], seg[1]))
            .fold(f64::MAX, f64::min),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: f64, lon: f64) -> Coordinate {
        Coordinate { lat, lon }
    }

    #[test]
    fn haversine_along_latitude() {
        let d = haversine_distance(c(54.795, 9.43), c(54.795, 9.44));
        assert!((d - 641.0).abs() < 2.0, "got {d}");
    }

    #[test]
    fn haversine_along_longitude_meridian() {
        let d = haversine_distance(c(54.795, 9.43), c(54.796, 9.43));
        assert!((d - 111.2).abs() < 0.5, "got {d}");
    }

    #[test]
    fn perpendicular_distance_to_segment() {
        let d = point_to_segment_distance(c(54.7955, 9.435), c(54.795, 9.43), c(54.795, 9.44));
        assert!((d - 55.6).abs() < 0.5, "got {d}");
    }

    #[test]
    fn degenerate_segment_falls_back_to_point_distance() {
        let d = point_to_segment_distance(c(54.796, 9.43), c(54.795, 9.43), c(54.795, 9.43));
        assert!((d - 111.2).abs() < 0.5, "got {d}");
    }

    #[test]
    fn line_distance_takes_minimum_over_segments() {
        let line = [c(54.795, 9.43), c(54.795, 9.44), c(54.80, 9.44)];
        let d = point_to_line_distance(c(54.7955, 9.435), &line);
        assert!((d - 55.6).abs() < 0.5, "got {d}");
    }

    #[test]
    fn line_with_single_point_uses_haversine() {
        let d = point_to_line_distance(c(54.796, 9.43), &[c(54.795, 9.43)]);
        assert!((d - 111.2).abs() < 0.5, "got {d}");
    }

    #[test]
    fn empty_line_is_infinitely_far() {
        assert_eq!(point_to_line_distance(c(54.795, 9.43), &[]), f64::MAX);
    }
}
