use crate::geometry::point_to_line_distance;
use crate::model::{Coordinate, NodeId, Way, WayId};
use rstar::RTree;
use rstar::primitives::{GeomWithData, Rectangle};
use std::collections::HashMap;

pub const MATCH_THRESHOLD_M: f64 = 50.0;
const NEAREST_CANDIDATES: usize = 10;

type IndexedBox = GeomWithData<Rectangle<[f64; 2]>, WayId>;

pub struct WayIndex {
    tree: RTree<IndexedBox>,
    geometries: HashMap<WayId, Vec<Coordinate>>,
}

impl WayIndex {
    pub fn build(ways: &[Way], nodes: &HashMap<NodeId, Coordinate>) -> Self {
        let mut boxes = Vec::new();
        let mut geometries = HashMap::new();

        for way in ways {
            let coords: Vec<Coordinate> = way
                .node_refs
                .iter()
                .filter_map(|id| nodes.get(id).copied())
                .collect();
            if coords.len() < 2 {
                continue;
            }

            let min_lon = coords.iter().map(|c| c.lon).fold(f64::INFINITY, f64::min);
            let max_lon = coords
                .iter()
                .map(|c| c.lon)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_lat = coords.iter().map(|c| c.lat).fold(f64::INFINITY, f64::min);
            let max_lat = coords
                .iter()
                .map(|c| c.lat)
                .fold(f64::NEG_INFINITY, f64::max);

            boxes.push(GeomWithData::new(
                Rectangle::from_corners([min_lon, min_lat], [max_lon, max_lat]),
                way.id,
            ));
            geometries.insert(way.id, coords);
        }

        Self {
            tree: RTree::bulk_load(boxes),
            geometries,
        }
    }

    pub fn match_point(&self, point: Coordinate) -> Option<(WayId, f64)> {
        let mut best: Option<(WayId, f64)> = None;
        for candidate in self
            .tree
            .nearest_neighbor_iter([point.lon, point.lat])
            .take(NEAREST_CANDIDATES)
        {
            let way_id = candidate.data;
            let dist = point_to_line_distance(point, &self.geometries[&way_id]);
            if dist < MATCH_THRESHOLD_M && best.is_none_or(|(_, d)| dist < d) {
                best = Some((way_id, dist));
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_network() -> (Vec<Way>, HashMap<NodeId, Coordinate>) {
        let nodes: HashMap<NodeId, Coordinate> = [
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
                    lat: 54.8100,
                    lon: 9.4700,
                },
            ),
            (
                6,
                Coordinate {
                    lat: 54.8100,
                    lon: 9.4710,
                },
            ),
        ]
        .into();
        let way = |id: WayId, refs: &[NodeId]| Way {
            id,
            version: 1,
            node_refs: refs.to_vec(),
            tags: vec![("highway".into(), "residential".into())],
        };
        (
            vec![way(100, &[1, 2, 3]), way(200, &[3, 4]), way(300, &[5, 6])],
            nodes,
        )
    }

    #[test]
    fn matches_nearest_way_within_threshold() {
        let (ways, nodes) = test_network();
        let index = WayIndex::build(&ways, &nodes);
        let (way_id, dist) = index
            .match_point(Coordinate {
                lat: 54.79505,
                lon: 9.4305,
            })
            .unwrap();
        assert_eq!(way_id, 100);
        assert!(dist < 10.0, "got {dist}");
    }

    #[test]
    fn no_match_beyond_threshold() {
        let (ways, nodes) = test_network();
        let index = WayIndex::build(&ways, &nodes);
        assert_eq!(
            index.match_point(Coordinate {
                lat: 54.90,
                lon: 9.60
            }),
            None
        );
    }

    #[test]
    fn ways_with_fewer_than_two_known_nodes_are_skipped() {
        let nodes: HashMap<NodeId, Coordinate> = [(
            1,
            Coordinate {
                lat: 54.7950,
                lon: 9.4300,
            },
        )]
        .into();
        let ways = vec![Way {
            id: 100,
            version: 1,
            node_refs: vec![1, 99],
            tags: vec![],
        }];
        let index = WayIndex::build(&ways, &nodes);
        assert_eq!(
            index.match_point(Coordinate {
                lat: 54.7950,
                lon: 9.4300
            }),
            None
        );
    }
}
