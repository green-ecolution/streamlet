use crate::model::{Coordinate, NodeId, Way, WayId};
use crate::spatial::WayIndex;
use crate::verkehrsticker::ConstructionSite;
use std::collections::{HashMap, HashSet};

pub fn changed_ways(
    sites: &[ConstructionSite],
    ways: &[Way],
    nodes: &HashMap<NodeId, Coordinate>,
) -> Vec<Way> {
    let index = WayIndex::build(ways, nodes);
    let matched: HashSet<WayId> = sites
        .iter()
        .filter_map(|site| {
            index
                .match_point(Coordinate {
                    lat: site.lat,
                    lon: site.lon,
                })
                .map(|(way_id, _)| way_id)
        })
        .collect();

    let way_by_id: HashMap<WayId, &Way> = ways.iter().map(|w| (w.id, w)).collect();

    let mut node_to_ways: HashMap<NodeId, Vec<WayId>> = HashMap::new();
    for way in ways {
        for node in &way.node_refs {
            node_to_ways.entry(*node).or_default().push(way.id);
        }
    }

    let mut changed: HashMap<WayId, Way> = HashMap::new();

    for way_id in &matched {
        let mut way = way_by_id[way_id].clone();
        way.tags.push(("access".into(), "no".into()));
        changed.insert(*way_id, way);
    }

    for way_id in &matched {
        for node in &way_by_id[way_id].node_refs {
            for neighbor_id in node_to_ways
                .get(node)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                if matched.contains(neighbor_id) || changed.contains_key(neighbor_id) {
                    continue;
                }
                let neighbor = way_by_id[neighbor_id];
                if matches!(neighbor.tag("oneway"), Some("yes") | Some("1") | Some("-1")) {
                    let mut way = neighbor.clone();
                    way.tags.retain(|(k, _)| k != "oneway");
                    changed.insert(*neighbor_id, way);
                }
            }
        }
    }

    let mut result: Vec<Way> = changed.into_values().collect();
    result.sort_by_key(|w| w.id);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(lat: f64, lon: f64) -> ConstructionSite {
        ConstructionSite {
            id: 1,
            name: "Teststraße".into(),
            lon,
            lat,
        }
    }

    fn way(id: WayId, refs: &[NodeId], tags: &[(&str, &str)]) -> Way {
        Way {
            id,
            version: 1,
            node_refs: refs.to_vec(),
            tags: tags
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    fn nodes() -> HashMap<NodeId, Coordinate> {
        [
            (
                1,
                Coordinate {
                    lat: 54.7950,
                    lon: 9.4300,
                },
            ),
            (
                2,
                Coordinate {
                    lat: 54.7950,
                    lon: 9.4310,
                },
            ),
            (
                3,
                Coordinate {
                    lat: 54.7950,
                    lon: 9.4320,
                },
            ),
            (
                4,
                Coordinate {
                    lat: 54.7960,
                    lon: 9.4320,
                },
            ),
            (
                5,
                Coordinate {
                    lat: 54.7960,
                    lon: 9.4300,
                },
            ),
            (
                6,
                Coordinate {
                    lat: 54.8100,
                    lon: 9.4700,
                },
            ),
            (
                7,
                Coordinate {
                    lat: 54.8100,
                    lon: 9.4710,
                },
            ),
        ]
        .into()
    }

    #[test]
    fn blocks_matched_way_and_opens_adjacent_oneway() {
        let ways = vec![
            way(
                100,
                &[1, 2, 3],
                &[("highway", "residential"), ("name", "Teststraße")],
            ),
            way(
                200,
                &[3, 4],
                &[("highway", "residential"), ("oneway", "yes")],
            ),
            way(
                400,
                &[1, 5],
                &[("highway", "residential"), ("oneway", "no")],
            ),
            way(
                300,
                &[6, 7],
                &[("highway", "residential"), ("oneway", "yes")],
            ),
        ];
        let changed = changed_ways(&[site(54.79505, 9.4305)], &ways, &nodes());

        assert_eq!(
            changed.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![100, 200]
        );

        let blocked = &changed[0];
        assert_eq!(blocked.tag("access"), Some("no"));
        assert_eq!(blocked.tag("name"), Some("Teststraße"));

        let opened = &changed[1];
        assert_eq!(opened.tag("oneway"), None);
        assert_eq!(opened.tag("highway"), Some("residential"));
        assert_eq!(opened.version, 1);
    }

    #[test]
    fn no_sites_means_no_changes() {
        let ways = vec![way(100, &[1, 2, 3], &[("highway", "residential")])];
        assert!(changed_ways(&[], &ways, &nodes()).is_empty());
    }

    #[test]
    fn unmatched_site_changes_nothing() {
        let ways = vec![way(100, &[1, 2, 3], &[("highway", "residential")])];
        assert!(changed_ways(&[site(54.99, 9.99)], &ways, &nodes()).is_empty());
    }

    #[test]
    fn does_not_recurse_beyond_direct_neighbors() {
        let mut n = nodes();
        n.insert(
            8,
            Coordinate {
                lat: 54.7970,
                lon: 9.4320,
            },
        );
        let ways = vec![
            way(100, &[1, 2, 3], &[("highway", "residential")]),
            way(
                200,
                &[3, 4],
                &[("highway", "residential"), ("oneway", "yes")],
            ),
            way(
                500,
                &[4, 8],
                &[("highway", "residential"), ("oneway", "yes")],
            ),
        ];
        let changed = changed_ways(&[site(54.79505, 9.4305)], &ways, &n);
        assert_eq!(
            changed.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![100, 200]
        );
    }

    #[test]
    fn opens_numeric_oneway_variants() {
        let ways = vec![
            way(100, &[1, 2, 3], &[("highway", "residential")]),
            way(200, &[3, 4], &[("highway", "residential"), ("oneway", "1")]),
            way(
                400,
                &[1, 5],
                &[("highway", "residential"), ("oneway", "-1")],
            ),
        ];
        let changed = changed_ways(&[site(54.79505, 9.4305)], &ways, &nodes());
        assert_eq!(
            changed.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![100, 200, 400]
        );
        assert_eq!(changed[1].tag("oneway"), None);
        assert_eq!(changed[2].tag("oneway"), None);
    }

    #[test]
    fn matched_way_keeps_its_own_oneway() {
        let ways = vec![way(
            100,
            &[1, 2, 3],
            &[("highway", "residential"), ("oneway", "yes")],
        )];
        let changed = changed_ways(&[site(54.79505, 9.4305)], &ways, &nodes());
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].tag("access"), Some("no"));
        assert_eq!(changed[0].tag("oneway"), Some("yes"));
    }
}
