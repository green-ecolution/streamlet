pub type NodeId = i64;
pub type WayId = i64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinate {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    pub id: WayId,
    pub version: i32,
    pub node_refs: Vec<NodeId>,
    pub tags: Vec<(String, String)>,
}

impl Way {
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

const CAR_ACCESSIBLE_HIGHWAYS: &[&str] = &[
    "motorway",
    "trunk",
    "primary",
    "secondary",
    "tertiary",
    "unclassified",
    "residential",
    "motorway_link",
    "trunk_link",
    "primary_link",
    "secondary_link",
    "tertiary_link",
    "living_street",
    "service",
    "road",
];

pub fn is_car_accessible(highway: &str) -> bool {
    CAR_ACCESSIBLE_HIGHWAYS.contains(&highway)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tag_by_key() {
        let way = Way {
            id: 1,
            version: 1,
            node_refs: vec![],
            tags: vec![
                ("highway".into(), "residential".into()),
                ("oneway".into(), "yes".into()),
            ],
        };
        assert_eq!(way.tag("oneway"), Some("yes"));
        assert_eq!(way.tag("access"), None);
    }

    #[test]
    fn classifies_car_accessible_highways() {
        assert!(is_car_accessible("residential"));
        assert!(is_car_accessible("living_street"));
        assert!(is_car_accessible("motorway_link"));
        assert!(!is_car_accessible("footway"));
        assert!(!is_car_accessible("cycleway"));
        assert!(!is_car_accessible(""));
    }
}
