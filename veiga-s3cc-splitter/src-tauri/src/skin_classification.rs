//! Evidence-based skin detection. Names and color-ramp references are NOT
//! proof of a default replacement. The catalog must come from a trusted
//! operator-supplied inventory of full EA-game resource TGIs.
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub(crate) const TONE: u32 = 0x0354796A;
pub(crate) const IMG: u32 = 0x00B2D882;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) struct SkinResourceKey {
    pub resource_type: u32,
    pub group: u32,
    pub instance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkinKind {
    DefaultReplacement,
    NonDefault,
    Mixed,
    Unverified,
}
impl SkinKind {
    pub(crate) fn display(self) -> &'static str {
        match self {
            Self::DefaultReplacement => "Default Replacement",
            Self::NonDefault => "Non-default",
            Self::Mixed => "Mixed",
            Self::Unverified => "Unverified",
        }
    }
}

pub(crate) struct EaSkinCatalog {
    known_ea: HashSet<SkinResourceKey>,
}

#[derive(Deserialize)]
struct JsonCatalog {
    resources: Vec<JsonTgi>,
}
#[derive(Deserialize)]
struct JsonTgi {
    #[serde(rename = "type")]
    resource_type: String,
    group: String,
    instance: String,
    #[serde(default)]
    role: String,
}

fn read_hex(s: &str, max_len: usize) -> Option<u64> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    if s.is_empty() || s.len() > max_len || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(s, 16).ok()
}

impl EaSkinCatalog {
    pub(crate) fn from_json(text: &str) -> Result<Self, String> {
        let data: JsonCatalog = serde_json::from_str(text)
            .map_err(|e| format!("Invalid EA skin catalog: {e}"))?;
        if data.resources.is_empty() {
            return Err("EA skin catalog has no entries".to_string());
        }
        let mut known_ea = HashSet::new();
        for resource in data.resources {
            let resource_type = read_hex(&resource.resource_type, 8)
                .ok_or("Invalid resource type in EA skin catalog")? as u32;
            let group = read_hex(&resource.group, 8)
                .ok_or("Invalid group in EA skin catalog")? as u32;
            let instance = read_hex(&resource.instance, 16)
                .ok_or("Invalid instance in EA skin catalog")?;
            if resource_type != TONE && resource_type != IMG {
                return Err("EA skin catalog includes an unsupported resource type".to_string());
            }
            let role = resource.role.trim();
            if !role.is_empty() && !role.eq_ignore_ascii_case(
                if resource_type == TONE { "TONE" } else { "IMG" }
            ) {
                return Err("EA skin catalog role does not match its full TGI".to_string());
            }
            known_ea.insert(SkinResourceKey { resource_type, group, instance });
        }
        Ok(Self { known_ea })
    }

    pub(crate) fn load_for_package(package_path: &Path) -> Option<Self> {
        let catalog_path: PathBuf = std::env::var_os("S3CC_EA_SKIN_REFERENCE_CATALOG")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| package_path.parent().unwrap_or(Path::new("."))
                .join("ea-skin-reference-catalog.json"));
        let data = std::fs::read_to_string(&catalog_path).ok()?;
        Self::from_json(&data).ok()
    }

    pub(crate) fn classify(&self, keys: &[SkinResourceKey]) -> SkinKind {
        let tones: Vec<_> = keys.iter().filter(|key| key.resource_type == TONE).collect();
        if !tones.is_empty() {
            let ea = tones.iter().any(|key| self.known_ea.contains(key));
            let custom = tones.iter().any(|key| !self.known_ea.contains(key));
            return match (ea, custom) {
                (true, true) => SkinKind::Mixed,
                (true, false) => SkinKind::DefaultReplacement,
                (false, true) => SkinKind::NonDefault,
                (false, false) => SkinKind::Unverified,
            };
        }
        // For IMG-only groups, a catalog hit proves replacement of an EA
        // resource. An IMG without a hit cannot by itself prove non-default.
        let images: Vec<_> = keys.iter().filter(|key| key.resource_type == IMG).collect();
        if images.iter().any(|key| self.known_ea.contains(key)) {
            SkinKind::DefaultReplacement
        } else {
            SkinKind::Unverified
        }
    }
}

pub(crate) fn classify_skin(
    catalog: Option<&EaSkinCatalog>,
    keys: &[SkinResourceKey],
) -> SkinKind {
    catalog.map(|ref_data| ref_data.classify(keys))
        .unwrap_or(SkinKind::Unverified)
}

#[cfg(test)]
mod tests {
    use super::*;
    const EA_JSON: &str = r#"{"resources":[
       {"type":"0x00B2D882","group":"0x00000000","instance":"0x6A00EC677E971DBC","role":"IMG"},
       {"type":"0x0354796A","group":"0x00000000","instance":"0x0000000000000044","role":"TONE"}
    ]}"#;
    fn key(ty: u32, group: u32, instance: u64) -> SkinResourceKey {
        SkinResourceKey { resource_type: ty, group, instance }
    }
    fn ea() -> EaSkinCatalog { EaSkinCatalog::from_json(EA_JSON).unwrap() }

    #[test]
    fn no_trusted_catalog_is_unverified() {
        assert_eq!(classify_skin(None, &[key(IMG,0,0x6A00EC677E971DBC)]),
            SkinKind::Unverified);
    }
    #[test]
    fn img_only_ea_tgi_is_default_replacement() {
        assert_eq!(ea().classify(&[key(IMG,0,0x6A00EC677E971DBC)]),
            SkinKind::DefaultReplacement);
    }
    #[test]
    fn ea_tone_full_tgi_is_default_replacement() {
        assert_eq!(ea().classify(&[key(TONE,0,0x44)]),
            SkinKind::DefaultReplacement);
    }
    #[test]
    fn custom_tone_remains_non_default_even_with_ea_img_ramp_dependency() {
        assert_eq!(ea().classify(&[key(TONE,0,0x95C9DE637D75A5AC),
                                  key(IMG,0,0x6A00EC677E971DBC)]),
            SkinKind::NonDefault);
    }
    #[test]
    fn mixed_tones_yield_mixed() {
        assert_eq!(ea().classify(&[key(TONE,0,0x44),key(TONE,0,0xFACE)]),
            SkinKind::Mixed);
    }
    #[test]
    fn same_instance_wrong_type_or_group_never_matches_catalog() {
        assert_eq!(ea().classify(&[key(IMG,1,0x6A00EC677E971DBC)]),
            SkinKind::Unverified);
        assert_eq!(ea().classify(&[key(IMG,0,0x44)]), SkinKind::Unverified);
    }
    #[test]
    fn unknown_img_without_ea_tgi_stays_unverified() {
        assert_eq!(ea().classify(&[key(IMG,0,0x999)]), SkinKind::Unverified);
    }
    #[test]
    fn reject_empty_or_malformed_catalog() {
        assert!(EaSkinCatalog::from_json(r#"{"resources":[]}"#).is_err());
        assert!(EaSkinCatalog::from_json(r#"{"resources":[{"type":"BAD","group":"0x0","instance":"0x1"}]}"#).is_err());
    }
}
