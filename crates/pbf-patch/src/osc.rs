use crate::model::Way;
use std::fmt::Write;

// osmium apply-changes requires the attribute to be present; the value is irrelevant.
const OSC_TIMESTAMP: &str = "2025-11-11T00:00:00Z";

pub fn write_osc(ways: &[Way]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<osmChange version=\"0.6\" generator=\"pbf-patch\">\n");
    out.push_str("  <modify>\n");

    for way in ways {
        writeln!(
            out,
            "    <way id=\"{}\" version=\"{}\" timestamp=\"{OSC_TIMESTAMP}\">",
            way.id,
            way.version + 1
        )
        .unwrap();
        for node in &way.node_refs {
            writeln!(out, "      <nd ref=\"{node}\"/>").unwrap();
        }
        for (key, value) in &way.tags {
            writeln!(
                out,
                "      <tag k=\"{}\" v=\"{}\"/>",
                escape_xml(key),
                escape_xml(value)
            )
            .unwrap();
        }
        out.push_str("    </way>\n");
    }

    out.push_str("  </modify>\n");
    out.push_str("</osmChange>\n");
    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_modify_block_with_escaping() {
        let ways = vec![Way {
            id: 100,
            version: 2,
            node_refs: vec![1, 2],
            tags: vec![
                ("highway".into(), "residential".into()),
                ("name".into(), "A & B <\"Straße\">".into()),
            ],
        }];
        let expected = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<osmChange version=\"0.6\" generator=\"pbf-patch\">\n",
            "  <modify>\n",
            "    <way id=\"100\" version=\"3\" timestamp=\"2025-11-11T00:00:00Z\">\n",
            "      <nd ref=\"1\"/>\n",
            "      <nd ref=\"2\"/>\n",
            "      <tag k=\"highway\" v=\"residential\"/>\n",
            "      <tag k=\"name\" v=\"A &amp; B &lt;&quot;Straße&quot;&gt;\"/>\n",
            "    </way>\n",
            "  </modify>\n",
            "</osmChange>\n",
        );
        assert_eq!(write_osc(&ways), expected);
    }

    #[test]
    fn empty_input_writes_empty_modify_block() {
        let osc = write_osc(&[]);
        assert!(osc.contains("<modify>"));
        assert!(!osc.contains("<way"));
    }
}
