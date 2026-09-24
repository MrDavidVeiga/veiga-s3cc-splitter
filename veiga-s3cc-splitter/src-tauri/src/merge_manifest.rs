use std::collections::HashSet;

use sims3_core::dbpf::DBPF;
use sims3_pack::dbpf::Package as WritablePackage;

pub const MANIFEST_TYPE: u32 = 0x0333406C;
pub const MANIFEST_GROUP: u32 = 0x56534733; // "VSG3"
pub const MANIFEST_INSTANCE: u64 = 0x56454947414D4E46; // "VEIGAMNF"

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestAnchor {
    pub kind: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestResource {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPackage {
    pub id: u32,
    pub name: String,
    pub original_size: u64,
    pub anchors: Vec<ManifestAnchor>,
    pub resources: Vec<ManifestResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeManifest {
    pub version: u32,
    pub packages: Vec<ManifestPackage>,
}

pub fn is_manifest_tgi(resource_type: u32, group: u32, instance: u64) -> bool {
    resource_type == MANIFEST_TYPE && group == MANIFEST_GROUP && instance == MANIFEST_INSTANCE
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(xml_unescape(&rest[..end]))
}

impl MergeManifest {
    pub fn to_xml_bytes(&self) -> Vec<u8> {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str(&format!(
            "<VeigaS3CCMergeManifest version=\"{}\" packageCount=\"{}\">\n",
            self.version,
            self.packages.len()
        ));
        for package in &self.packages {
            out.push_str(&format!(
                "  <Package id=\"{}\" name=\"{}\" originalSize=\"{}\" resourceCount=\"{}\">\n",
                package.id,
                xml_escape(&package.name),
                package.original_size,
                package.resources.len()
            ));
            for anchor in &package.anchors {
                out.push_str(&format!(
                    "    <Anchor kind=\"{}\" key=\"{}\" />\n",
                    xml_escape(&anchor.kind),
                    xml_escape(&anchor.key)
                ));
            }
            for resource in &package.resources {
                out.push_str(&format!(
                    "    <Resource key=\"{}\" />\n",
                    xml_escape(&resource.key)
                ));
            }
            out.push_str("  </Package>\n");
        }
        out.push_str("</VeigaS3CCMergeManifest>\n");
        out.into_bytes()
    }

    pub fn from_xml(data: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(data)
            .map_err(|_| "Merge Manifest não está em UTF-8 válido.".to_string())?;
        let root_pos = text
            .find("<VeigaS3CCMergeManifest")
            .ok_or_else(|| "Merge Manifest inválido: raiz não encontrada.".to_string())?;
        let root_end = text[root_pos..]
            .find('>')
            .map(|v| root_pos + v + 1)
            .ok_or_else(|| "Merge Manifest inválido: raiz incompleta.".to_string())?;
        let root_tag = &text[root_pos..root_end];
        let version = attr(root_tag, "version")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);

        let mut packages = Vec::new();
        let mut cursor = root_end;
        while let Some(rel_start) = text[cursor..].find("<Package ") {
            let start = cursor + rel_start;
            let open_end = text[start..]
                .find('>')
                .map(|v| start + v + 1)
                .ok_or_else(|| "Merge Manifest inválido: Package incompleto.".to_string())?;
            let close = text[open_end..]
                .find("</Package>")
                .map(|v| open_end + v)
                .ok_or_else(|| "Merge Manifest inválido: fechamento de Package ausente.".to_string())?;
            let tag = &text[start..open_end];
            let body = &text[open_end..close];

            let id = attr(tag, "id")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or((packages.len() + 1) as u32);
            let name = attr(tag, "name").unwrap_or_else(|| format!("Package_{id}.package"));
            let original_size = attr(tag, "originalSize")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            let mut anchors = Vec::new();
            let mut anchor_cursor = 0usize;
            while let Some(rel) = body[anchor_cursor..].find("<Anchor ") {
                let a_start = anchor_cursor + rel;
                let a_end = body[a_start..]
                    .find('>')
                    .map(|v| a_start + v + 1)
                    .ok_or_else(|| "Merge Manifest inválido: Anchor incompleto.".to_string())?;
                let a_tag = &body[a_start..a_end];
                if let (Some(kind), Some(key)) = (attr(a_tag, "kind"), attr(a_tag, "key")) {
                    anchors.push(ManifestAnchor { kind, key });
                }
                anchor_cursor = a_end;
            }

            let mut resources = Vec::new();
            let mut resource_cursor = 0usize;
            while let Some(rel) = body[resource_cursor..].find("<Resource ") {
                let r_start = resource_cursor + rel;
                let r_end = body[r_start..]
                    .find('>')
                    .map(|v| r_start + v + 1)
                    .ok_or_else(|| "Merge Manifest inválido: Resource incompleto.".to_string())?;
                let r_tag = &body[r_start..r_end];
                if let Some(key) = attr(r_tag, "key") {
                    resources.push(ManifestResource { key });
                }
                resource_cursor = r_end;
            }

            packages.push(ManifestPackage {
                id,
                name,
                original_size,
                anchors,
                resources,
            });
            cursor = close + "</Package>".len();
        }

        Ok(Self { version, packages })
    }

    pub fn update_after_removal(
        &self,
        selected_keys: &[String],
        removed_keys: &HashSet<String>,
    ) -> Self {
        let selected: HashSet<String> = selected_keys
            .iter()
            .flat_map(|key| key.split("||"))
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let mut packages = Vec::new();
        for package in &self.packages {
            let remove_package = package
                .anchors
                .iter()
                .any(|anchor| selected.contains(&anchor.key) || removed_keys.contains(&anchor.key));
            if remove_package {
                continue;
            }

            let mut updated = package.clone();
            updated
                .resources
                .retain(|resource| !removed_keys.contains(&resource.key));
            updated
                .anchors
                .retain(|anchor| !removed_keys.contains(&anchor.key));
            if updated.resources.is_empty() {
                continue;
            }
            packages.push(updated);
        }

        Self {
            version: self.version,
            packages,
        }
    }

    pub fn validate_keys(&self, available_keys: &HashSet<String>) -> Result<(), String> {
        for package in &self.packages {
            for anchor in &package.anchors {
                if !available_keys.contains(&anchor.key) {
                    return Err(format!(
                        "Merge Manifest inválido: anchor {} de {} não existe no package.",
                        anchor.key, package.name
                    ));
                }
            }
            for resource in &package.resources {
                if !available_keys.contains(&resource.key) {
                    return Err(format!(
                        "Merge Manifest inválido: recurso {} de {} não existe no package.",
                        resource.key, package.name
                    ));
                }
            }
        }
        Ok(())
    }
}

pub fn read_from_dbpf(package: &DBPF<'_>) -> Result<Option<MergeManifest>, String> {
    let Some(entry) = package.files.iter().find(|entry| {
        is_manifest_tgi(entry.resource_type, entry.resource_group, entry.instance)
    }) else {
        return Ok(None);
    };
    MergeManifest::from_xml(entry.data()).map(Some)
}

pub fn read_from_writable(package: &WritablePackage) -> Result<Option<MergeManifest>, String> {
    let Some(entry) = package.entries.iter().find(|entry| {
        is_manifest_tgi(entry.type_id, entry.group, entry.instance)
    }) else {
        return Ok(None);
    };
    let data = package
        .get_uncompressed_data(entry)
        .map_err(|e| format!("Não foi possível ler o Merge Manifest: {e}"))?;
    MergeManifest::from_xml(&data).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_manifest_xml() {
        let manifest = MergeManifest {
            version: 1,
            packages: vec![ManifestPackage {
                id: 1,
                name: "TShirt & Mickey.package".to_string(),
                original_size: 1234,
                anchors: vec![ManifestAnchor {
                    kind: "CASP".to_string(),
                    key: "0x034AEECB-0x00000000-0x1234567890ABCDEF".to_string(),
                }],
                resources: vec![ManifestResource {
                    key: "0x034AEECB-0x00000000-0x1234567890ABCDEF".to_string(),
                }],
            }],
        };
        let xml = manifest.to_xml_bytes();
        let parsed = MergeManifest::from_xml(&xml).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn removal_drops_selected_package_but_keeps_shared_resource_owner() {
        let shared = "0x00B2D882-0x00000000-0x00000000000000AA".to_string();
        let a_anchor = "0x034AEECB-0x00000000-0x0000000000000001".to_string();
        let b_anchor = "0x034AEECB-0x00000000-0x0000000000000002".to_string();
        let manifest = MergeManifest {
            version: 1,
            packages: vec![
                ManifestPackage {
                    id: 1,
                    name: "A.package".to_string(),
                    original_size: 1,
                    anchors: vec![ManifestAnchor { kind: "CASP".into(), key: a_anchor.clone() }],
                    resources: vec![
                        ManifestResource { key: a_anchor.clone() },
                        ManifestResource { key: shared.clone() },
                    ],
                },
                ManifestPackage {
                    id: 2,
                    name: "B.package".to_string(),
                    original_size: 1,
                    anchors: vec![ManifestAnchor { kind: "CASP".into(), key: b_anchor.clone() }],
                    resources: vec![
                        ManifestResource { key: b_anchor.clone() },
                        ManifestResource { key: shared.clone() },
                    ],
                },
            ],
        };
        let mut removed = HashSet::new();
        removed.insert(a_anchor.clone());
        let updated = manifest.update_after_removal(&[a_anchor], &removed);
        assert_eq!(updated.packages.len(), 1);
        assert_eq!(updated.packages[0].name, "B.package");
        assert!(updated.packages[0].resources.iter().any(|r| r.key == shared));
    }
}
