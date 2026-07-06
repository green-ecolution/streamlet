use crate::model::{Coordinate, NodeId, Way, is_car_accessible};
use anyhow::Result;
use osmpbf::{Element, ElementReader};
use std::collections::HashMap;
use std::path::Path;

pub fn load_car_network(path: &Path) -> Result<(HashMap<NodeId, Coordinate>, Vec<Way>)> {
    let reader = ElementReader::from_path(path)?;
    let mut nodes = HashMap::new();
    let mut ways = Vec::new();

    reader.for_each(|element| match element {
        Element::Node(n) => {
            nodes.insert(n.id(), Coordinate { lat: n.lat(), lon: n.lon() });
        }
        Element::DenseNode(n) => {
            nodes.insert(n.id(), Coordinate { lat: n.lat(), lon: n.lon() });
        }
        Element::Way(w) => {
            let highway = w.tags().find(|(k, _)| *k == "highway").map(|(_, v)| v);
            if highway.is_some_and(is_car_accessible) {
                ways.push(Way {
                    id: w.id(),
                    version: w.info().version().unwrap_or(0),
                    node_refs: w.refs().collect(),
                    tags: w
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                });
            }
        }
        Element::Relation(_) => {}
    })?;

    Ok((nodes, ways))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_nodes_and_car_accessible_ways() {
        let (nodes, ways) =
            load_car_network(Path::new("tests/fixtures/flensburg.osm.pbf")).unwrap();

        assert_eq!(nodes.len(), 6);
        assert!((nodes[&1].lat - 54.7950).abs() < 1e-7);
        assert!((nodes[&1].lon - 9.4300).abs() < 1e-7);

        let ids: Vec<_> = ways.iter().map(|w| w.id).collect();
        assert_eq!(ids, vec![100, 200], "footway 300 must be filtered out");

        let way100 = &ways[0];
        assert_eq!(way100.version, 2);
        assert_eq!(way100.node_refs, vec![1, 2, 3]);
        assert_eq!(way100.tag("name"), Some("Teststraße"));
    }
}
