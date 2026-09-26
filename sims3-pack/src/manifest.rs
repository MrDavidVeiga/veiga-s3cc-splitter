use anyhow::{bail, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Minimal fields used by the original tools.
#[derive(Debug, Clone, Default)]
pub struct ManifestInfo {
    pub paid_content: bool,
    pub package_title: Option<String>,
    /// Key string formatted like `0xTTTTTTTT-0xGGGGGGGG-0xIIIIIIIIIIIIIIII`
    pub thumbnail_key: Option<String>,
}

pub fn parse_manifest(xml: &str) -> Result<ManifestInfo> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut info = ManifestInfo::default();
    let mut in_manifest = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if tag == "manifest" {
                    in_manifest = true;
                    // attributes
                    for a in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(a.key.as_ref()).to_ascii_lowercase();
                        if k == "paidcontent" {
                            let v = a.unescape_value().unwrap_or_default().to_ascii_lowercase();
                            if v == "true" {
                                info.paid_content = true;
                            }
                        }
                    }
                } else if in_manifest {
                    let val = reader.read_text(e.name()).unwrap_or_default();
                    match tag.as_str() {
                        "packagetitle" => {
                            let cleaned = val.trim().to_string();
                            if !cleaned.is_empty() {
                                info.package_title = Some(cleaned);
                            }
                        }
                        "thumbnail" => {
                            // Original is "TYPE:GROUP:INSTANCE" hex without 0x prefix.
                            let parts: Vec<String> = val
                                .trim()
                                .to_uppercase()
                                .split(':')
                                .map(|s| s.to_string())
                                .collect();
                            if parts.len() == 3 {
                                let t = format!("0x{:0>8}", parts[0]);
                                let g = format!("0x{:0>8}", parts[1]);
                                let i = format!("0x{:0>16}", parts[2]);
                                info.thumbnail_key = Some(format!("{t}-{g}-{i}"));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if tag == "manifest" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(info)
}
