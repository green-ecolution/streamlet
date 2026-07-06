use anyhow::{Context, Result, bail};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructionSite {
    pub id: i64,
    pub name: String,
    pub lon: f64,
    pub lat: f64,
}

const TICKER_URL: &str = "https://tbz-flensburg.de/de/verkehrsticker";
const SECTION_IDS: [&str; 4] = ["neu", "geaendert", "unveraendert", "geplant"];
const GEO_DATA_PREFIX: &str = "geoData = '";
const GEO_DATA_SUFFIX: &str = "'; //console.log";

#[derive(Deserialize)]
struct GeoJson {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Feature {
    geometry: Geometry,
    properties: Properties,
}

#[derive(Deserialize)]
struct Geometry {
    coordinates: Vec<f64>,
}

#[derive(Deserialize)]
struct Properties {
    id: i64,
    name: String,
    akz: String,
}

pub fn fetch() -> Result<Vec<ConstructionSite>> {
    let body = reqwest::blocking::get(TICKER_URL)?
        .error_for_status()?
        .text()?;
    parse(&body)
}

pub fn parse(html: &str) -> Result<Vec<ConstructionSite>> {
    let doc = Html::parse_document(html);
    let geo_data = extract_geo_data(&doc)?;
    let states = extract_states(&doc)?;

    let parsed: GeoJson = serde_json::from_str(&geo_data).context("invalid geoData JSON")?;

    let mut sites = Vec::new();
    for feature in parsed.features {
        if feature.geometry.coordinates.len() < 2 {
            continue;
        }
        if states.get(&feature.properties.akz).map(String::as_str) == Some("geplant") {
            continue;
        }
        if feature.properties.name.contains("Teilsperrung") {
            continue;
        }
        sites.push(ConstructionSite {
            id: feature.properties.id,
            name: feature.properties.name,
            lon: feature.geometry.coordinates[0],
            lat: feature.geometry.coordinates[1],
        });
    }
    Ok(sites)
}

fn extract_geo_data(doc: &Html) -> Result<String> {
    let selector = Selector::parse("script").unwrap();
    for script in doc.select(&selector) {
        let text: String = script.text().collect();
        if let Some(rest) = text.strip_prefix(GEO_DATA_PREFIX)
            && let Some((json, _)) = rest.split_once(GEO_DATA_SUFFIX)
        {
            return Ok(json.to_string());
        }
    }
    bail!("no geoData found on Verkehrsticker page")
}

fn extract_states(doc: &Html) -> Result<HashMap<String, String>> {
    let akz_selector = Selector::parse(".meldung_rh > a").unwrap();

    let first_section = SECTION_IDS.iter().find_map(|id| {
        let selector = Selector::parse(&format!("#{id}")).unwrap();
        doc.select(&selector).next()
    });
    let Some(first) = first_section else {
        bail!("no report sections found on Verkehrsticker page")
    };

    let mut states = HashMap::new();
    let mut current_state = first
        .value()
        .attr("id")
        .expect("selected by id")
        .to_string();

    let mut current = first.next_sibling();
    while let Some(node) = current {
        current = node.next_sibling();
        let Some(element) = ElementRef::wrap(node) else {
            continue;
        };
        if element.value().name() == "h2" {
            let id = element
                .value()
                .attr("id")
                .context("h2 without id in report list")?;
            if !SECTION_IDS.contains(&id) {
                bail!("unexpected section id in report list: {id}");
            }
            current_state = id.to_string();
            continue;
        }
        let akz = element
            .select(&akz_selector)
            .next()
            .context("report entry without akz anchor")?;
        let name = akz
            .value()
            .attr("name")
            .context("akz anchor without name attribute")?;
        states.insert(name.to_string(), current_state.clone());
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_filters_verkehrsticker() {
        let html = include_str!("../tests/fixtures/verkehrsticker.html");
        let sites = parse(html).unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].id, 1);
        assert_eq!(sites[0].name, "Duburger Straße");
        assert!((sites[0].lon - 9.430680).abs() < 1e-9);
        assert!((sites[0].lat - 54.795919).abs() < 1e-9);
    }

    #[test]
    fn fails_without_geo_data() {
        let err = parse("<html><body></body></html>").unwrap_err();
        assert!(err.to_string().contains("geoData"));
    }
}
