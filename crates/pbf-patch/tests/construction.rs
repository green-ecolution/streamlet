use pbf_patch::construction::changed_ways;
use pbf_patch::osc::write_osc;
use pbf_patch::pbf::load_car_network;
use pbf_patch::verkehrsticker::ConstructionSite;
use std::path::Path;

#[test]
fn patches_fixture_network() {
    let (nodes, ways) = load_car_network(Path::new("tests/fixtures/flensburg.osm.pbf")).unwrap();

    let sites = vec![ConstructionSite {
        id: 1,
        name: "Teststraße".into(),
        lon: 9.4305,
        lat: 54.79505,
    }];

    let changed = changed_ways(&sites, &ways, &nodes);
    assert_eq!(
        changed.iter().map(|w| w.id).collect::<Vec<_>>(),
        vec![100, 200]
    );

    let osc = write_osc(&changed);
    assert!(osc.contains(r#"<way id="100" version="3""#));
    assert!(osc.contains(r#"<tag k="access" v="no"/>"#));

    let way200 = &osc[osc.find(r#"<way id="200""#).unwrap()..];
    assert!(!way200.contains("oneway"));
}
