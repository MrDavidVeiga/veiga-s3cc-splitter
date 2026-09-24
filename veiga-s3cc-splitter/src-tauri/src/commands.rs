use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::convert::TryInto;
use std::fs::{copy, create_dir_all, remove_file, rename, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use memmap::Mmap;
use num_traits::ToPrimitive;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sims3_core::dbpf::filetypes::{resource_is_png, ResourceType};
use sims3_core::dbpf::{DBPFEntry, DBPF};
use sims3_pack::dbpf::{Package as WritablePackage, ResourceEntry as WritableEntry};
use zip::write::FileOptions;

use crate::merge_manifest::{
    is_manifest_tgi, read_from_dbpf as read_merge_manifest_from_dbpf,
    read_from_writable as read_merge_manifest_from_writable, ManifestAnchor, ManifestPackage,
    ManifestResource, MergeManifest, MANIFEST_GROUP, MANIFEST_INSTANCE, MANIFEST_TYPE,
};

const TYPE_CASP: u32 = 0x034AEECB;
const TYPE_FACE: u32 = 0x0358B08A;
const TYPE_SKINTONE: u32 = 0x0354796A;
const TYPE_PATTERN: u32 = 0xD4D9FBE5;
const TYPE_XML: u32 = 0x0333406C;
const TYPE_THUM_SMALL: u32 = 0x626F60CC;
const TYPE_THUM_MED: u32 = 0x626F60CD;
const TYPE_THUM_LARGE: u32 = 0x626F60CE;
const TYPE_IMG: u32 = 0x00B2D882;
const TYPE_IMAG_JPG: u32 = 0x2F7D0002;
const TYPE_IMAG_PNG: u32 = 0x2F7D0004;
const TYPE_IMAG_TGA: u32 = 0x2F7D0006;
const TYPE_GEOM: u32 = 0x015A1849;
const TYPE_VPXY: u32 = 0x736884F1;
const TYPE_MODL: u32 = 0x01661233;
const TYPE_KEY: u32 = 0x0166038C;
const TYPE_MLOD: u32 = 0x01D10F34;
const TYPE_MTST: u32 = 0x02019972;
const TYPE_BONE: u32 = 0x00AE6C67;
const TYPE_TXTC: u32 = 0x033A1435;
const TYPE_BOND: u32 = 0x0355E0A6;
const TYPE_BBLN: u32 = 0x062C8204;
const TYPE_BGEO: u32 = 0x067CAA11;
const TYPE_STBL: u32 = 0x220557DA;
const TYPE_FBLN: u32 = 0xB52F5055;
const TYPE_OBJD: u32 = 0x319E4F1D;
const TYPE_OBJK: u32 = 0x02DC343F;
const TYPE_RSLT: u32 = 0xD3044521;
const TYPE_FTPT: u32 = 0xD382BF57;
const TYPE_LITE: u32 = 0x03B4C61D;
const TYPE_RIG: u32 = 0x8EAF13DE;
const TYPE_ICON: u32 = 0x2E75C765;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPayload {
    pub package_path: String,
    #[serde(default = "default_scan_resource_type")]
    pub resource_type: String,
    pub search: Option<String>,
}

fn default_scan_resource_type() -> String {
    "all".to_string()
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResourceItem {
    pub label: String,
    pub key: String,
    pub kind: String,
    pub details: String,
    #[serde(default)]
    pub content_categories: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub items: Vec<ResourceItem>,
    pub manifest_package_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub package_path: String,
    pub resource_key: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub label: String,
    pub key: String,
    pub kind: String,
    pub details: String,
    pub thumb_base64: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelectionPayload {
    pub package_path: String,
    pub destination_folder: String,
    pub selected_keys: Vec<String>,
    pub include_thumbnails: bool,
    pub enable_compression: bool,
    pub create_clone_package: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MergePackagesPayload {
    pub package_paths: Vec<String>,
    pub destination_folder: String,
    pub output_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePackagesResult {
    pub output_path: String,
    pub package_count: usize,
    pub resource_count: usize,
    pub shared_resources: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RestoreManifestPayload {
    pub package_path: String,
    pub destination_folder: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreManifestResult {
    pub destination_folder: String,
    pub package_count: usize,
    pub output_paths: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSelectionPayload {
    pub package_path: String,
    pub selected_keys: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSelectionApplyPayload {
    pub package_path: String,
    pub destination_folder: String,
    pub selected_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSelectionPlan {
    pub selected_items: usize,
    pub selected_resources: usize,
    pub candidate_resources: usize,
    pub shared_resources_preserved: usize,
    pub selected_resources_preserved: usize,
    pub resources_to_remove: usize,
    pub remaining_resources: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSelectionResult {
    pub output_path: String,
    pub backup_path: String,
    pub removed_resources: usize,
    pub shared_resources_preserved: usize,
    pub selected_resources_preserved: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ResourceKeyParts {
    resource_type: u32,
    resource_group: u32,
    instance: u64,
}

fn open_package(path: &str) -> Result<Mmap, String> {
    let file = File::open(path).map_err(|e| format!("Não foi possível abrir o arquivo: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }
        .map_err(|e| format!("Não foi possível mapear o arquivo: {e}"))?;

    if mmap.len() < 4 {
        return Err("Arquivo inválido: muito pequeno para ser um package DBPF.".to_string());
    }

    if &mmap[0..4] != b"DBPF" {
        return Err("Arquivo inválido: não é um package DBPF válido.".to_string());
    }

    Ok(mmap)
}

fn parse_package(mmap: &Mmap) -> Result<DBPF<'_>, String> {
    DBPF::new(&mmap[..]).map_err(|e| format!("Package inválido: {e:?}"))
}

fn kind_from_type(resource_type: u32) -> &'static str {
    match resource_type {
        TYPE_CASP => "CASP",
        TYPE_FACE => "FACE",
        TYPE_SKINTONE => "SkinTone",
        TYPE_PATTERN => "Pattern",
        TYPE_XML => "XML",
        TYPE_THUM_SMALL | TYPE_THUM_MED | TYPE_THUM_LARGE => "THUM",
        TYPE_IMG => "IMG",
        TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA => "IMAG",
        TYPE_GEOM => "GEOM",
        TYPE_VPXY => "VPXY",
        TYPE_MODL => "MODL",
        TYPE_KEY => "KEY",
        TYPE_MLOD => "MLOD",
        TYPE_MTST => "MTST",
        TYPE_BONE => "BONE",
        TYPE_TXTC => "TXTC",
        TYPE_BOND => "BOND",
        TYPE_BBLN => "BBLN",
        TYPE_BGEO => "BGEO",
        TYPE_FBLN => "FBLN",
        TYPE_OBJD => "OBJD",
        TYPE_OBJK => "OBJK",
        TYPE_RSLT => "RSLT",
        TYPE_FTPT => "FTPT",
        TYPE_LITE => "LITE",
        TYPE_RIG => "RIG",
        TYPE_ICON => "ICON",
        _ => "Resource",
    }
}

fn extension_from_type(resource_type: u32) -> &'static str {
    match resource_type {
        TYPE_CASP => "casp",
        TYPE_FACE => "face",
        TYPE_SKINTONE => "skintone",
        TYPE_PATTERN => "pattern",
        TYPE_XML => "xmlbin",
        TYPE_THUM_SMALL | TYPE_THUM_MED | TYPE_THUM_LARGE => "thum",
        TYPE_IMG => "img",
        TYPE_IMAG_JPG => "jpg",
        TYPE_IMAG_PNG => "png",
        TYPE_IMAG_TGA => "tga",
        TYPE_GEOM => "geom",
        TYPE_VPXY => "vpxy",
        TYPE_MODL => "modl",
        TYPE_KEY => "key",
        TYPE_MLOD => "mlod",
        TYPE_MTST => "mtst",
        TYPE_BONE => "bone",
        TYPE_TXTC => "txtc",
        TYPE_BOND => "bond",
        TYPE_BBLN => "bbln",
        TYPE_BGEO => "bgeo",
        TYPE_FBLN => "fbln",
        TYPE_OBJD => "object",
        TYPE_OBJK => "objkey",
        TYPE_RSLT => "slot",
        TYPE_FTPT => "footprint",
        TYPE_LITE => "light",
        TYPE_RIG => "grannyrig",
        TYPE_ICON => "png",
        _ => "bin",
    }
}

fn key_string(entry: &DBPFEntry<'_>) -> String {
    format!(
        "0x{:08X}-0x{:08X}-0x{:016X}",
        entry.resource_type, entry.resource_group, entry.instance
    )
}

fn key_string_from_parts(parts: ResourceKeyParts) -> String {
    format!(
        "0x{:08X}-0x{:08X}-0x{:016X}",
        parts.resource_type, parts.resource_group, parts.instance
    )
}

fn key_string_from_writable(entry: &WritableEntry) -> String {
    format!(
        "0x{:08X}-0x{:08X}-0x{:016X}",
        entry.type_id, entry.group, entry.instance
    )
}

fn parse_key(key: &str) -> Result<ResourceKeyParts, String> {
    let key = first_compound_key(key);
    let mut pieces = key.split('-');
    let resource_type = u32::from_str_radix(
        pieces
            .next()
            .ok_or("Missing type")?
            .trim_start_matches("0x"),
        16,
    )
    .map_err(|e| e.to_string())?;
    let resource_group = u32::from_str_radix(
        pieces
            .next()
            .ok_or("Missing group")?
            .trim_start_matches("0x"),
        16,
    )
    .map_err(|e| e.to_string())?;
    let instance = u64::from_str_radix(
        pieces
            .next()
            .ok_or("Missing instance")?
            .trim_start_matches("0x"),
        16,
    )
    .map_err(|e| e.to_string())?;
    Ok(ResourceKeyParts {
        resource_type,
        resource_group,
        instance,
    })
}

fn filter_matches(entry: &DBPFEntry<'_>, mode: &str) -> bool {
    match mode {
        "casp" => entry.resource_type == ResourceType::CASP.to_u32().unwrap(),
        "face" => entry.resource_type == ResourceType::FACE.to_u32().unwrap(),
        "skintone" => entry.resource_type == ResourceType::SkinTone.to_u32().unwrap(),
        "pattern" => entry.resource_type == ResourceType::PTRN.to_u32().unwrap(),
        "object" => entry.resource_type == TYPE_OBJD,
        _ => matches!(
            entry.resource_type,
            TYPE_CASP | TYPE_FACE | TYPE_SKINTONE | TYPE_PATTERN | TYPE_XML | TYPE_OBJD
        ),
    }
}

fn first_compound_key(key: &str) -> &str {
    key.split("||").next().unwrap_or(key)
}

fn expand_compound_keys(keys: &HashSet<String>) -> HashSet<String> {
    let mut expanded = HashSet::new();
    for key in keys {
        for part in key.split("||") {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                expanded.insert(trimmed.to_string());
            }
        }
    }
    expanded
}

fn normalized_tone_name(name: &str) -> String {
    let re = Regex::new(r"(?i)_custom_\d+$").unwrap();
    re.replace(name.trim(), "").to_string()
}

fn is_bodyhair_label(name: &str) -> bool {
    let lower = name.trim().to_lowercase();
    lower.starts_with("bhair")
        || lower.contains("bodyhair")
        || lower.contains("armhair")
        || lower.contains("bellyhair")
        || lower.contains("chestbellyhair")
        || lower.contains("chesthair")
        || lower.contains("leghair")
        || lower.contains("lowerback")
        || lower.contains("upperback")
        || lower.contains("fullback")
        || lower.contains("legs-calves_hair")
        || lower.contains("arms_hair")
        || lower.contains("belly_hair")
        || lower.contains("chest_hair")
}

fn normalized_bodyhair_name(name: &str) -> String {
    let s = name.trim();

    let patterns = [
        r"(?i)(?:[_\-\s]?)(?:armhair|bellyhair|chestbellyhair|chesthair|leghair|lowerback|upperback|fullback)(?:[_\-\s]?\d+)?$",
        r"(?i)(?:[_\-\s]?)(?:arms[_\-\s]?hair|belly[_\-\s]?hair|chest[_\-\s]?hair|legs[_\-\s]?calves[_\-\s]?hair)$",
    ];

    for pat in patterns {
        let re = Regex::new(pat).unwrap();
        let candidate = re
            .replace(s, "")
            .trim_end_matches(['_', '-', ' '])
            .trim()
            .to_string();
        if !candidate.is_empty() && candidate.len() < s.len() {
            return candidate;
        }
    }

    s.to_string()
}

fn build_casp_group_items(
    package: &DBPF<'_>,
    query: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceItem> {
    let mut bodyhair_groups: HashMap<String, Vec<&DBPFEntry<'_>>> = HashMap::new();
    let mut singles: Vec<ResourceItem> = Vec::new();

    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_CASP)
    {
        let label = object_display_label(package, entry, name_map);
        if is_bodyhair_label(&label) {
            let base = normalized_bodyhair_name(&label);
            bodyhair_groups.entry(base).or_default().push(entry);
        } else {
            let item = build_item(package, entry, name_map);
            if item_matches_query(&item, query) {
                singles.push(item);
            }
        }
    }

    let mut items = Vec::new();
    for (base, mut entries) in bodyhair_groups {
        entries.sort_by_key(|entry| entry.instance);
        let first = entries[0];
        let joined_key = entries
            .iter()
            .map(|entry| key_string(entry))
            .collect::<Vec<_>>()
            .join("||");
        let label = format!("{}-BodyHair", base);
        let details = format!(
            "{} arquivos body hair agrupados • Type 0x{:08X} • Group 0x{:08X}",
            entries.len(),
            first.resource_type,
            first.resource_group
        );
        let item = ResourceItem {
            label,
            key: joined_key,
            kind: kind_from_type(first.resource_type).to_string(),
            details,
            content_categories: vec!["Body Hair".to_string()],
        };
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    items.extend(singles);
    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    items
}

#[derive(Clone, Debug)]
struct CaspFolderInfo {
    main_category: String,
    sub_category: Option<String>,
    age_folder: String,
    gender_folder: String,
    use_age_folder: bool,
    use_gender_folder: bool,
}

fn casp_parse_core_fields(data: &[u8]) -> Option<(u32, u32, u32)> {
    if data.len() < 12 {
        return None;
    }
    let mut offset = 8usize;
    let preset_count = read_le_u32(data, offset)? as usize;
    offset += 4;
    if preset_count > 4096 {
        return None;
    }
    for _ in 0..preset_count {
        let xml_len = read_le_i32(data, offset)? as usize;
        offset += 4;
        let xml_bytes = xml_len.checked_mul(2)?;
        let xml_end = offset.checked_add(xml_bytes)?;
        if xml_len > 1_000_000 || xml_end > data.len() {
            return None;
        }
        offset = xml_end;
        offset = offset.checked_add(4)?;
        if offset > data.len() {
            return None;
        }
    }
    let _unknown1_name = read_be_7bit_string(data, &mut offset)?;
    offset = offset.checked_add(4)?;
    offset = offset.checked_add(1)?;
    if offset + 12 > data.len() {
        return None;
    }
    let clothing = read_le_u32(data, offset)?;
    offset += 4;
    let data_type = read_le_u32(data, offset)?;
    offset += 4;
    let age_gender = read_le_u32(data, offset)?;
    Some((clothing, data_type, age_gender))
}

fn casp_age_folder(age_bits: u32) -> String {
    let known_mask = 0x01 | 0x02 | 0x04 | 0x08 | 0x10 | 0x20 | 0x40;
    let bits = age_bits & known_mask;
    if bits == (0x10 | 0x20) {
        return "Adult".to_string();
    }
    let mut names = Vec::new();
    if (bits & 0x01) != 0 {
        names.push("Baby");
    }
    if (bits & 0x02) != 0 {
        names.push("Toddler");
    }
    if (bits & 0x04) != 0 {
        names.push("Child");
    }
    if (bits & 0x08) != 0 {
        names.push("Teen");
    }
    if (bits & 0x10) != 0 {
        names.push("YoungAdult");
    }
    if (bits & 0x20) != 0 {
        names.push("Adult");
    }
    if (bits & 0x40) != 0 {
        names.push("Elder");
    }
    if names.is_empty() {
        "UnknownAge".to_string()
    } else if names.len() == 1 {
        names[0].to_string()
    } else {
        names.join("-")
    }
}

fn casp_gender_folder(age_gender: u32) -> String {
    let gender_bits = (age_gender >> 12) & 0x3;
    match gender_bits {
        0x1 => "Male".to_string(),
        0x2 => "Female".to_string(),
        0x3 => "Unisex".to_string(),
        _ => "UnknownGender".to_string(),
    }
}

fn casp_makeup_subcategory(clothing: u32) -> Option<&'static str> {
    match clothing {
        0x00000011 => Some("Lipstick"),
        0x00000012 => Some("Eyeshadow"),
        0x00000013 => Some("Eyeliner"),
        0x00000014 => Some("Blush"),
        0x00000015 => Some("Makeup"),
        0x0000001A => Some("Mascara"),
        0x0000001B => Some("Moles"),
        0x0000001C => Some("Freckles"),
        0x0000001D => Some("Weathering"),
        0x00000021 => Some("Tattoo"),
        0x00000022 => Some("TattooTemplate"),
        0x00000023 => Some("Dental"),
        0x00000026 => Some("BirthMark"),
        _ => None,
    }
}

fn casp_accessories_subcategory(clothing: u32) -> Option<&'static str> {
    match clothing {
        0x00000009 => Some("Necklace"),
        0x0000000A => Some("NoseRing"),
        0x0000000B | 0x0000001E | 0x0000001F => Some("Earrings"),
        0x0000000C => Some("Glasses"),
        0x0000000D => Some("Bracelets"),
        0x0000000E | 0x0000000F => Some("Rings"),
        0x00000018 => Some("Gloves"),
        0x00000019 => Some("Socks"),
        0x00000020 => Some("ArmBand"),
        0x00000024 | 0x00000025 => Some("Garters"),
        _ => None,
    }
}

fn casp_folder_info(
    entry: &DBPFEntry<'_>,
    name_map: &BTreeMap<u64, String>,
) -> Option<CaspFolderInfo> {
    if entry.resource_type != TYPE_CASP {
        return None;
    }
    let (clothing, data_type, age_gender) = casp_parse_core_fields(entry.data())?;
    let label = name_map.get(&entry.instance).cloned().unwrap_or_default();
    let is_body_hair = matches!(clothing, 0x00000027..=0x0000002E) || is_bodyhair_label(&label);
    if is_body_hair {
        return Some(CaspFolderInfo {
            main_category: "Hair".to_string(),
            sub_category: Some("Body Hair".to_string()),
            age_folder: String::new(),
            gender_folder: String::new(),
            use_age_folder: false,
            use_gender_folder: false,
        });
    }

    let age_folder = casp_age_folder(age_gender & 0x7F);
    let gender_folder = casp_gender_folder(age_gender);

    let (main_category, sub_category, use_age_folder, use_gender_folder) =
        if let Some(sub) = casp_makeup_subcategory(clothing) {
            ("Makeup".to_string(), Some(sub.to_string()), true, true)
        } else if let Some(sub) = casp_accessories_subcategory(clothing) {
            ("Accessories".to_string(), Some(sub.to_string()), true, true)
        } else {
            match clothing {
                0x00000001 => ("Hair".to_string(), Some("Hair".to_string()), true, true),
                0x00000003 => ("Hair".to_string(), Some("Hair".to_string()), true, true),
                0x00000010 => ("Hair".to_string(), Some("Beard".to_string()), false, false),
                0x00000016 => (
                    "Hair".to_string(),
                    Some("Eyebrows".to_string()),
                    true,
                    false,
                ),
                0x00000004 | 0x00000005 | 0x00000006 | 0x00000007 => {
                    ("Clothing".to_string(), None, true, true)
                }
                _ => {
                    if (data_type & 0x00000010) != 0 {
                        (
                            "Accessories".to_string(),
                            Some("Accessories".to_string()),
                            true,
                            true,
                        )
                    } else if (data_type & 0x00000001) != 0 {
                        ("Hair".to_string(), Some("Hair".to_string()), true, true)
                    } else if (data_type & 0x00000008) != 0 {
                        ("Clothing".to_string(), None, true, true)
                    } else {
                        return None;
                    }
                }
            }
        };

    Some(CaspFolderInfo {
        main_category,
        sub_category,
        age_folder,
        gender_folder,
        use_age_folder,
        use_gender_folder,
    })
}

fn push_unique_category(categories: &mut Vec<String>, category: &str) {
    if !categories.iter().any(|existing| existing == category) {
        categories.push(category.to_string());
    }
}

fn casp_content_category(info: Option<CaspFolderInfo>, label: &str) -> &'static str {
    let lowered = label.to_ascii_lowercase();

    if lowered.contains("shoe") || lowered.contains("boot") || lowered.contains("sandal") {
        return "Shoes";
    }
    if lowered.contains("eye") && !lowered.contains("eyebrow") {
        return "Eyes";
    }
    if lowered.contains("face") {
        return "Face Parts";
    }
    if lowered.contains("pet") && lowered.contains("coat") {
        return "Pet Coats";
    }
    if lowered.contains("pet") && lowered.contains("accessor") {
        return "Pet Accessories";
    }
    if lowered.contains("pet") {
        return "Pet Parts";
    }

    let Some(info) = info else {
        return "Other CASP";
    };

    match (info.main_category.as_str(), info.sub_category.as_deref()) {
        ("Hair", Some("Body Hair")) => "Body Hair",
        ("Hair", Some("Beard")) => "Facial Hair",
        ("Hair", Some("Eyebrows")) => "Eyebrows",
        ("Hair", _) => "Hair",
        ("Clothing", _) => "Clothes",
        ("Accessories", _) => "Accessories",
        ("Makeup", Some("Moles" | "Freckles" | "Weathering" | "BirthMark")) => "Skin Details",
        ("Makeup", Some("Tattoo" | "TattooTemplate")) => "Tattoos",
        ("Makeup", Some("Dental")) => "Teeth",
        ("Makeup", _) => "Makeup",
        _ => "Other CASP",
    }
}

fn object_content_category(label: &str) -> &'static str {
    let lowered = label.to_ascii_lowercase();
    let contains_any = |needles: &[&str]| needles.iter().any(|needle| lowered.contains(needle));

    if contains_any(&["door", "arch"]) {
        "Doors & Arches"
    } else if contains_any(&["window"]) {
        "Windows"
    } else if contains_any(&["fence", "gate"]) {
        "Fences & Gates"
    } else if contains_any(&["stair", "railing"]) {
        "Stairs & Railings"
    } else if contains_any(&["column"]) {
        "Columns"
    } else if contains_any(&["foundation"]) {
        "Foundations"
    } else if contains_any(&["wall"]) {
        "Walls"
    } else if contains_any(&["floor"]) {
        "Floors"
    } else if contains_any(&["ceiling"]) {
        "Ceilings"
    } else if contains_any(&["roof"]) {
        "Roofs"
    } else if contains_any(&["fireplace", "chimney"]) {
        "Fireplaces & Chimneys"
    } else if contains_any(&["plant", "tree", "rock", "shrub"]) {
        "Plants & Rocks"
    } else if contains_any(&["terrain paint"]) {
        "Terrain Paints"
    } else if contains_any(&["terrain"]) {
        "Terrain Tools"
    } else if contains_any(&["pool", "fountain"]) {
        "Pools & Fountains"
    } else if contains_any(&["elevator"]) {
        "Elevators"
    } else if contains_any(&["rabbit hole", "rabbithole"]) {
        "Rabbit Holes"
    } else if contains_any(&["shell"]) {
        "Building Shells"
    } else if contains_any(&["landmark"]) {
        "Landmarks"
    } else if contains_any(&["blueprint"]) {
        "Blueprints"
    } else if contains_any(&["resort"]) {
        "Resort"
    } else if contains_any(&["debug"]) {
        "Debug"
    } else if contains_any(&["bed", "chair", "sofa", "loveseat", "seat", "bench"]) {
        "Comfort"
    } else if contains_any(&["table", "desk", "counter", "surface", "shelf"]) {
        "Surfaces"
    } else if contains_any(&["dresser", "cabinet", "storage", "bookcase"]) {
        "Storage"
    } else if contains_any(&["fridge", "stove", "oven", "dishwasher", "washer", "dryer"]) {
        "Appliances"
    } else if contains_any(&["tv", "stereo", "computer", "phone", "electronic"]) {
        "Electronics"
    } else if contains_any(&["toilet", "sink", "shower", "tub", "bath", "plumbing"]) {
        "Plumbing"
    } else if contains_any(&["lamp", "light", "lighting"]) {
        "Lighting"
    } else if contains_any(&["decor", "painting", "sculpture", "rug", "curtain", "mirror", "plant"]) {
        "Decor"
    } else if contains_any(&["game", "arcade", "instrument", "entertainment"]) {
        "Entertainment"
    } else if contains_any(&["toy", "crib", "kids", "child"]) {
        "Kids"
    } else if contains_any(&["car", "bike", "vehicle", "boat"]) {
        "Vehicles"
    } else if contains_any(&["pet"]) {
        "Pets"
    } else if contains_any(&["stage", "show"]) {
        "Show Stage"
    } else {
        "Other Objects"
    }
}

fn content_categories_for_key(
    package: &DBPF<'_>,
    key: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<String> {
    let mut categories = Vec::new();
    let Ok(parts) = parse_key(key) else {
        return categories;
    };

    match parts.resource_type {
        TYPE_FACE => push_unique_category(&mut categories, "Sliders"),
        TYPE_SKINTONE => push_unique_category(&mut categories, "Skins"),
        TYPE_CASP => {
            let entry = package.files.iter().find(|entry| {
                entry.resource_type == parts.resource_type
                    && entry.resource_group == parts.resource_group
                    && entry.instance == parts.instance
            });

            if let Some(entry) = entry {
                let label = object_display_label(package, entry, name_map);
                push_unique_category(
                    &mut categories,
                    casp_content_category(casp_folder_info(entry, name_map), &label),
                );
            } else {
                push_unique_category(&mut categories, "Other CASP");
            }
        }
        TYPE_OBJD => {
            let label = package
                .files
                .iter()
                .find(|entry| {
                    entry.resource_type == parts.resource_type
                        && entry.resource_group == parts.resource_group
                        && entry.instance == parts.instance
                })
                .map(|entry| object_display_label(package, entry, name_map))
                .unwrap_or_default();
            push_unique_category(&mut categories, object_content_category(&label));
        }
        _ => {}
    }

    categories
}

fn selection_export_root(
    package: &DBPF<'_>,
    destination_folder: &str,
    base_key: &str,
    name_map: &BTreeMap<u64, String>,
) -> PathBuf {
    let stem = selection_output_stem(package, base_key, name_map);
    if let Ok(parts) = parse_key(first_compound_key(base_key)) {
        if parts.resource_type == TYPE_FACE {
            return PathBuf::from(destination_folder).join("Sliders").join(stem);
        }
        if let Some(entry) = package.files.iter().find(|e| {
            e.resource_type == parts.resource_type
                && e.resource_group == parts.resource_group
                && e.instance == parts.instance
        }) {
            if let Some(info) = casp_folder_info(entry, name_map) {
                let mut path = PathBuf::from(destination_folder);
                path.push(info.main_category);
                if let Some(sub) = info.sub_category {
                    path.push(sub);
                }
                if info.use_age_folder && !info.age_folder.is_empty() {
                    path.push(info.age_folder);
                }
                if info.use_gender_folder && !info.gender_folder.is_empty() {
                    path.push(info.gender_folder);
                }
                path.push(stem);
                return path;
            }
        }
    }
    PathBuf::from(destination_folder).join(stem)
}

fn selection_reveal_root(
    package: &DBPF<'_>,
    destination_folder: &str,
    base_key: &str,
    name_map: &BTreeMap<u64, String>,
) -> PathBuf {
    if let Ok(parts) = parse_key(first_compound_key(base_key)) {
        if parts.resource_type == TYPE_FACE {
            return PathBuf::from(destination_folder).join("Sliders");
        }
        if let Some(entry) = package.files.iter().find(|e| {
            e.resource_type == parts.resource_type
                && e.resource_group == parts.resource_group
                && e.instance == parts.instance
        }) {
            if let Some(info) = casp_folder_info(entry, name_map) {
                return PathBuf::from(destination_folder).join(info.main_category);
            }
        }
    }
    PathBuf::from(destination_folder)
}

fn multi_export_root(destination_folder: &str, package_path: &str) -> PathBuf {
    let package_name = Path::new(package_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Export");

    PathBuf::from(destination_folder).join(package_name)
}

fn read_7bit_encoded_int(data: &[u8], offset: &mut usize) -> Option<usize> {
    let mut value: usize = 0;
    let mut shift = 0usize;
    loop {
        let byte = *data.get(*offset)?;
        *offset += 1;
        value |= ((byte & 0x7F) as usize) << shift;
        if (byte & 0x80) == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > (usize::BITS as usize - 7) {
            return None;
        }
    }
}

fn read_be_7bit_string(data: &[u8], offset: &mut usize) -> Option<String> {
    let byte_len = read_7bit_encoded_int(data, offset)?;
    if byte_len == 0 {
        return Some(String::new());
    }
    let end = offset.checked_add(byte_len)?;
    let raw = data.get(*offset..end)?;
    if raw.len() % 2 != 0 {
        return None;
    }
    let mut words = Vec::with_capacity(raw.len() / 2);
    for chunk in raw.chunks_exact(2) {
        words.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    *offset = end;
    Some(String::from_utf16_lossy(&words))
}

fn casp_unknown1_name(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }

    let mut offset = 8usize; // version + tgiPosn

    let preset_count = read_le_u32(data, offset)? as usize;
    offset += 4;

    if preset_count > 4096 {
        return None;
    }

    for _ in 0..preset_count {
        let xml_len = read_le_i32(data, offset)? as usize;
        offset += 4;

        let xml_bytes = xml_len.checked_mul(2)?;
        let xml_end = offset.checked_add(xml_bytes)?;
        if xml_len > 1_000_000 || xml_end > data.len() {
            return None;
        }
        offset = xml_end;

        // preset unknown1
        offset = offset.checked_add(4)?;
        if offset > data.len() {
            return None;
        }
    }

    let name = read_be_7bit_string(data, &mut offset)?;
    let trimmed = name.trim_matches('\0').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn raw_package_key_names(package: &DBPF<'_>) -> BTreeMap<u64, String> {
    let mut names = BTreeMap::new();
    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_KEY)
    {
        let data = entry.data();
        if data.len() < 8 {
            continue;
        }
        let Some(count) = read_le_u32(data, 4).map(|v| v as usize) else {
            continue;
        };
        if count > 100_000 {
            continue;
        }
        let mut offset = 8usize;
        for _ in 0..count {
            if offset + 12 > data.len() {
                break;
            }
            let Some(instance) = read_le_u64(data, offset) else {
                break;
            };
            let Some(length) = read_le_u32(data, offset + 8).map(|v| v as usize) else {
                break;
            };
            offset += 12;
            if length > 4096 || offset + length > data.len() {
                break;
            }
            let raw = &data[offset..offset + length];
            offset += length;
            let name = String::from_utf8_lossy(raw)
                .trim_matches('\0')
                .trim()
                .to_string();
            if !name.is_empty() {
                names.entry(instance).or_insert(name);
            }
        }
    }
    names
}

fn normalized_key_resource_name(name: &str) -> String {
    let original = name.trim_matches('\0').trim();
    if original.is_empty() {
        return String::new();
    }

    let mut current = original;
    loop {
        let Some((base, suffix)) = current.rsplit_once('_') else {
            break;
        };
        if base.is_empty() || suffix.is_empty() {
            break;
        }

        let token_end = suffix
            .find(|ch: char| ch.is_ascii_digit())
            .unwrap_or(suffix.len());
        let token = suffix[..token_end].to_ascii_uppercase();
        let numeric = &suffix[token_end..];
        let numeric_ok = numeric.is_empty() || numeric.chars().all(|ch| ch.is_ascii_digit());

        let technical = matches!(
            token.as_str(),
            "BONE"
                | "GEOM"
                | "IMG"
                | "IMAG"
                | "LOD"
                | "MLOD"
                | "MODL"
                | "VPXY"
                | "TXTC"
                | "BGEO"
                | "FBLN"
                | "BBLN"
                | "BOND"
                | "RIG"
                | "ICON"
                | "THUM"
                | "XML"
        );

        if !technical || !numeric_ok {
            break;
        }
        current = base;
    }

    current.to_string()
}

fn safe_package_resource_names(package: &DBPF<'_>) -> BTreeMap<u64, String> {
    let mut names: BTreeMap<u64, String> = raw_package_key_names(package)
        .into_iter()
        .map(|(instance, name)| (instance, normalized_key_resource_name(&name)))
        .filter(|(_, name)| !name.is_empty())
        .collect();

    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_CASP)
    {
        if names
            .get(&entry.instance)
            .map(|n| !n.trim().is_empty())
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(name) = casp_unknown1_name(entry.data()) {
            names.insert(entry.instance, name);
        }
    }

    // For objects, promote the same fallbacks used by the visible label into
    // the shared name map so exported package/file names benefit as well.
    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_OBJD)
    {
        if names
            .get(&entry.instance)
            .map(|n| !n.trim().is_empty())
            .unwrap_or(false)
        {
            continue;
        }

        let fallback = objd_catalog_name(entry.data())
            .or_else(|| {
                package.files.iter().find_map(|candidate| {
                    if candidate.resource_type != TYPE_XML || candidate.instance != entry.instance {
                        return None;
                    }
                    let text = String::from_utf8_lossy(candidate.data());
                    extract_xml_value_attr(&text, "daeFileName")
                })
            })
            .or_else(|| {
                package.files.iter().find_map(|candidate| {
                    if candidate.resource_type != TYPE_OBJK || candidate.instance != entry.instance {
                        return None;
                    }
                    objk_script_class_name(candidate.data())
                })
            });

        if let Some(name) = fallback {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                names.insert(entry.instance, trimmed.to_string());
            }
        }
    }

    if let Ok(Some(manifest)) = read_merge_manifest_from_dbpf(package) {
        for source in &manifest.packages {
            let stem = Path::new(&source.name)
                .file_stem()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or(&source.name)
                .to_string();
            for anchor in &source.anchors {
                if let Ok(parts) = parse_key(&anchor.key) {
                    names.insert(parts.instance, stem.clone());
                }
            }
        }
    }

    names
}

fn build_filtered_key_data(package: &DBPF<'_>, selected_keys: &HashSet<String>) -> Vec<u8> {
    // Preserve the original KEY strings exactly when rebuilding package metadata.
    // Display/export naming may normalize technical suffixes, but KEY metadata should not.
    let name_map = raw_package_key_names(package);
    let mut filtered: BTreeMap<u64, String> = BTreeMap::new();

    for key in selected_keys {
        let Ok(parts) = parse_key(key) else {
            continue;
        };
        if parts.resource_type == TYPE_KEY {
            continue;
        }
        let Some(name) = name_map.get(&parts.instance) else {
            continue;
        };
        let trimmed = name.trim_matches('\0').trim();
        if trimmed.is_empty() {
            continue;
        }
        filtered
            .entry(parts.instance)
            .or_insert_with(|| trimmed.to_string());
    }

    let mut out =
        Vec::with_capacity(8 + filtered.values().map(|name| 12 + name.len()).sum::<usize>());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(filtered.len() as u32).to_le_bytes());
    for (instance, name) in filtered {
        let raw = name.as_bytes();
        out.extend_from_slice(&instance.to_le_bytes());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(raw);
    }
    out
}

fn has_type_key(selected_keys: &HashSet<String>) -> bool {
    selected_keys.iter().any(|key| {
        parse_key(key)
            .map(|parts| parts.resource_type == TYPE_KEY)
            .unwrap_or(false)
    })
}

fn sanitize_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => out.push('_'),
            c if c.is_control() => out.push('_'),
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "resource".to_string()
    } else {
        trimmed.to_string()
    }
}

fn export_file_name(entry: &DBPFEntry<'_>, name_map: &BTreeMap<u64, String>) -> String {
    let base = name_map
        .get(&entry.instance)
        .cloned()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "{}-{}",
                kind_from_type(entry.resource_type),
                key_string(entry).replace(':', "_")
            )
        });
    let base = if entry.resource_type == TYPE_SKINTONE {
        let stem = Path::new(&base)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| base.clone());
        normalized_tone_name(&stem)
    } else {
        base
    };
    format!(
        "{}.{}",
        sanitize_file_name(&base),
        extension_from_type(entry.resource_type)
    )
}

fn export_file_stem(entry: &DBPFEntry<'_>, name_map: &BTreeMap<u64, String>) -> String {
    let file_name = export_file_name(entry, name_map);
    Path::new(&file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or(file_name)
}

fn tone_display_name(entry: &DBPFEntry<'_>, name_map: &BTreeMap<u64, String>) -> String {
    normalized_tone_name(&export_file_stem(entry, name_map))
}

fn normalized_face_name(name: &str) -> String {
    let mut s = name.trim().to_string();
    loop {
        let trimmed = s.trim_end();
        let mut changed = false;
        for suffix in ["Left", "Right"] {
            if trimmed.len() > suffix.len()
                && trimmed.to_lowercase().ends_with(&suffix.to_lowercase())
            {
                s = trimmed[..trimmed.len() - suffix.len()]
                    .trim_end_matches(['_', '-', ' '])
                    .to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    s
}

fn normalized_face_bundle_name(name: &str) -> String {
    let mut s = name.trim().to_string();
    loop {
        let trimmed = s.trim_end();
        let lower = trimmed.to_lowercase();
        let mut changed = false;
        for suffix in ["male", "female", "left", "right"] {
            if lower.ends_with(suffix) && trimmed.len() > suffix.len() {
                s = trimmed[..trimmed.len() - suffix.len()]
                    .trim_end_matches(['_', '-', ' '])
                    .to_string();
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }
    s
}

fn text_matches_face_bundle(text: &str, bundle_base: &str) -> bool {
    let target = normalized_face_name(bundle_base).trim().to_lowercase();
    if target.is_empty() {
        return false;
    }

    for piece in text.split(|c: char| c == '\0' || c == '\n' || c == '\r') {
        let normalized = normalized_face_name(piece).trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if normalized == target || normalized.contains(&target) {
            return true;
        }
    }

    false
}

fn extract_ascii_runs(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = Vec::new();

    for &b in data {
        let printable = matches!(b, 0x20..=0x7E) || b == b'\t';
        if printable {
            buf.push(b);
        } else {
            if buf.len() >= 4 {
                out.push(String::from_utf8_lossy(&buf).to_string());
            }
            buf.clear();
        }
    }

    if buf.len() >= 4 {
        out.push(String::from_utf8_lossy(&buf).to_string());
    }

    out
}

fn extract_utf16le_runs(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut words: Vec<u16> = Vec::new();

    for chunk in data.chunks_exact(2) {
        let w = u16::from_le_bytes([chunk[0], chunk[1]]);
        let printable = matches!(w, 0x20..=0x7E) || w == 0x0009;
        if printable {
            words.push(w);
        } else {
            if words.len() >= 4 {
                out.push(String::from_utf16_lossy(&words));
            }
            words.clear();
        }
    }

    if words.len() >= 4 {
        out.push(String::from_utf16_lossy(&words));
    }

    out
}

fn hex16_upper(value: u64) -> String {
    format!("{value:016X}")
}

fn hex16_lower(value: u64) -> String {
    format!("{value:016x}")
}

fn contains_u64_le(data: &[u8], value: u64) -> bool {
    let needle = value.to_le_bytes();
    data.windows(8).any(|w| w == needle)
}

fn contains_u64_be(data: &[u8], value: u64) -> bool {
    let needle = value.to_be_bytes();
    data.windows(8).any(|w| w == needle)
}

fn contains_utf16le_text(data: &[u8], needle: &str) -> bool {
    if needle.is_empty() || data.len() < needle.len() * 2 {
        return false;
    }

    let mut utf16 = Vec::with_capacity(needle.len() * 2);
    for w in needle.encode_utf16() {
        utf16.extend_from_slice(&w.to_le_bytes());
    }

    data.windows(utf16.len()).any(|w| w == utf16.as_slice())
}

fn stbl_references_instance_textually(data: &[u8], instance: u64) -> bool {
    let hex_upper = hex16_upper(instance);
    let hex_lower = hex16_lower(instance);

    let utf8 = String::from_utf8_lossy(data);
    if utf8.contains(&hex_upper) || utf8.contains(&hex_lower) {
        return true;
    }

    if data.windows(2).any(|pair| pair[1] == 0) {
        let utf16 = utf16le_lossy(data);
        if utf16.contains(&hex_upper) || utf16.contains(&hex_lower) {
            return true;
        }
    }

    contains_utf16le_text(data, &hex_upper) || contains_utf16le_text(data, &hex_lower)
}

fn stbl_references_fbln_instance(data: &[u8], fbln_instance: u64) -> bool {
    if stbl_references_instance_textually(data, fbln_instance) {
        return true;
    }

    if contains_u64_le(data, fbln_instance) || contains_u64_be(data, fbln_instance) {
        return true;
    }

    false
}

fn stbl_matches_face_bundle_or_fbln(
    data: &[u8],
    bundle_base: &str,
    fbln_instances: &[u64],
) -> bool {
    for &instance in fbln_instances {
        if stbl_references_fbln_instance(data, instance) {
            return true;
        }
    }

    for s in extract_ascii_runs(data) {
        if text_matches_face_bundle(&s, bundle_base) {
            return true;
        }
    }

    for s in extract_utf16le_runs(data) {
        if text_matches_face_bundle(&s, bundle_base) {
            return true;
        }
    }

    false
}

fn face_group_members(
    package: &DBPF<'_>,
    face: ResourceKeyParts,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceKeyParts> {
    let Some(entry) = package.files.iter().find(|entry| {
        entry.resource_type == face.resource_type
            && entry.resource_group == face.resource_group
            && entry.instance == face.instance
    }) else {
        return Vec::new();
    };

    let bundle_base = normalized_face_bundle_name(&resource_label(entry, name_map));
    let mut found: HashSet<ResourceKeyParts> = HashSet::new();
    let mut vpxy_instances: HashSet<u64> = HashSet::new();
    let allowed_face_refs = [TYPE_FACE, TYPE_BGEO, TYPE_FBLN, TYPE_VPXY];

    let mut seed_parts = Vec::new();
    let mut fbln_instances: HashSet<u64> = HashSet::new();

    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_FACE)
    {
        if normalized_face_bundle_name(&resource_label(candidate, name_map))
            .eq_ignore_ascii_case(&bundle_base)
        {
            let parts = ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            };
            seed_parts.push(parts);
            found.insert(parts);
            for companion in same_instance_companions(package, parts, false) {
                found.insert(companion);
            }
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_BGEO)
    {
        if normalized_face_bundle_name(&resource_label(candidate, name_map))
            .eq_ignore_ascii_case(&bundle_base)
        {
            found.insert(ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            });
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_FBLN)
    {
        if normalized_face_bundle_name(&resource_label(candidate, name_map))
            .eq_ignore_ascii_case(&bundle_base)
        {
            let parts = ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            };
            seed_parts.push(parts);
            found.insert(parts);
            fbln_instances.insert(parts.instance);
        }
    }

    // Follow direct references from FACE/FBLN resources instead of relying only on names.
    for seed in &seed_parts {
        if let Some(idx) = matching_entry_index(package, *seed) {
            let data = package.files[idx].data();
            for dep in tgi_refs_from_data(package, data, &allowed_face_refs) {
                if dep.resource_type == TYPE_VPXY {
                    vpxy_instances.insert(dep.instance);
                }
                if dep.resource_type == TYPE_FBLN {
                    fbln_instances.insert(dep.instance);
                }
                found.insert(dep);
                for companion in same_instance_companions(package, dep, false) {
                    found.insert(companion);
                }
            }
            for dep in textual_key_refs_from_data(package, data, &allowed_face_refs) {
                if dep.resource_type == TYPE_VPXY {
                    vpxy_instances.insert(dep.instance);
                }
                if dep.resource_type == TYPE_FBLN {
                    fbln_instances.insert(dep.instance);
                }
                found.insert(dep);
                for companion in same_instance_companions(package, dep, false) {
                    found.insert(companion);
                }
            }
        }
    }

    // Keep the name-based sweep too, because some merged packages rely on labels instead of direct refs.
    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_VPXY)
    {
        if normalized_face_bundle_name(&resource_label(candidate, name_map))
            .eq_ignore_ascii_case(&bundle_base)
        {
            let parts = ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            };
            found.insert(parts);
            vpxy_instances.insert(candidate.instance);
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_BOND)
    {
        if vpxy_instances.contains(&candidate.instance) {
            found.insert(ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            });
        }
    }

    // Preserve resource names on export.
    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_KEY)
    {
        found.insert(ResourceKeyParts {
            resource_type: candidate.resource_type,
            resource_group: candidate.resource_group,
            instance: candidate.instance,
        });
    }

    // Include only the STBL resources that belong to this slider bundle.
    let fbln_instances: Vec<u64> = fbln_instances.into_iter().collect();
    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_STBL)
    {
        if stbl_matches_face_bundle_or_fbln(candidate.data(), &bundle_base, &fbln_instances) {
            found.insert(ResourceKeyParts {
                resource_type: candidate.resource_type,
                resource_group: candidate.resource_group,
                instance: candidate.instance,
            });
        }
    }

    found.into_iter().collect()
}

fn expand_selected_for_faces(
    package: &DBPF<'_>,
    selected: &HashSet<String>,
    name_map: &BTreeMap<u64, String>,
) -> HashSet<String> {
    let mut expanded = expand_compound_keys(selected);
    let base_keys: Vec<String> = expanded.iter().cloned().collect();
    for key in base_keys {
        if let Ok(parts) = parse_key(&key) {
            if parts.resource_type == TYPE_FACE {
                for dep in face_group_members(package, parts, name_map) {
                    expanded.insert(key_string_from_parts(dep));
                }
            }
        }
    }
    expanded
}

fn build_face_group_items(
    package: &DBPF<'_>,
    query: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceItem> {
    let mut groups: HashMap<String, Vec<&DBPFEntry<'_>>> = HashMap::new();

    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_FACE)
    {
        let label = resource_label(entry, name_map);
        let base = normalized_face_name(&label);
        groups.entry(base).or_default().push(entry);
    }

    let mut items = Vec::new();
    for (base, mut entries) in groups {
        entries.sort_by_key(|entry| entry.instance);
        let first = entries[0];
        let joined_key = entries
            .iter()
            .map(|entry| key_string(entry))
            .collect::<Vec<_>>()
            .join("||");
        let details = if entries.len() > 1 {
            format!(
                "{} arquivos FACE agrupados • Type 0x{:08X} • Group 0x{:08X}",
                entries.len(),
                first.resource_type,
                first.resource_group
            )
        } else {
            format!(
                "Type 0x{:08X} • Group 0x{:08X}",
                first.resource_type, first.resource_group
            )
        };
        let item = ResourceItem {
            label: base,
            key: joined_key,
            kind: kind_from_type(first.resource_type).to_string(),
            details,
            content_categories: vec!["Sliders".to_string()],
        };
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    items
}

fn tone_group_members(
    package: &DBPF<'_>,
    tone: ResourceKeyParts,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceKeyParts> {
    let Some(entry) = package.files.iter().find(|entry| {
        entry.resource_type == tone.resource_type
            && entry.resource_group == tone.resource_group
            && entry.instance == tone.instance
    }) else {
        return Vec::new();
    };

    let base = tone_display_name(entry, name_map);
    let mut found = HashSet::new();

    let tone_instances: Vec<u64> = package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_SKINTONE)
        .filter(|candidate| tone_display_name(candidate, name_map).eq_ignore_ascii_case(&base))
        .map(|candidate| {
            found.insert(key_string(candidate));
            candidate.instance
        })
        .collect();

    for candidate in package.files.iter().filter(|candidate| {
        matches!(
            candidate.resource_type,
            TYPE_IMG | TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA
        )
    }) {
        let same_instance = tone_instances.contains(&candidate.instance);
        let same_name_family = name_map
            .get(&candidate.instance)
            .map(|name| {
                let lowered = name.to_lowercase();
                let lowered_base = base.to_lowercase();
                normalized_tone_name(name).eq_ignore_ascii_case(&base)
                    || lowered.starts_with(&lowered_base)
            })
            .unwrap_or(false);
        if same_instance || same_name_family {
            found.insert(key_string(candidate));
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_KEY)
    {
        found.insert(key_string(candidate));
    }

    let mut parts = Vec::new();
    for key in found {
        if let Ok(p) = parse_key(&key) {
            parts.push(p);
        }
    }
    parts
}

fn expand_selected_for_tones(
    package: &DBPF<'_>,
    selected: &HashSet<String>,
    name_map: &BTreeMap<u64, String>,
) -> HashSet<String> {
    let mut expanded = expand_compound_keys(selected);
    let base_keys: Vec<String> = expanded.iter().cloned().collect();
    for key in base_keys {
        if let Ok(parts) = parse_key(&key) {
            if parts.resource_type == TYPE_SKINTONE {
                for dep in tone_group_members(package, parts, name_map) {
                    expanded.insert(key_string_from_parts(dep));
                }
            }
        }
    }
    expanded
}

fn package_resource_names(package: &DBPF<'_>) -> BTreeMap<u64, String> {
    safe_package_resource_names(package)
}

fn read_7bit_encoded_usize(cur: &mut Cursor<&[u8]>) -> Option<usize> {
    let mut result: usize = 0;
    let mut shift = 0usize;
    loop {
        if shift >= usize::BITS as usize {
            return None;
        }
        let mut b = [0u8; 1];
        cur.read_exact(&mut b).ok()?;
        result |= ((b[0] & 0x7F) as usize) << shift;
        if (b[0] & 0x80) == 0 {
            return Some(result);
        }
        shift += 7;
    }
}

fn read_be_unicode_string(cur: &mut Cursor<&[u8]>) -> Option<String> {
    let byte_len = read_7bit_encoded_usize(cur)?;
    if byte_len == 0 {
        return Some(String::new());
    }
    if byte_len % 2 != 0 {
        return None;
    }

    let pos = cur.position() as usize;
    let end = pos.checked_add(byte_len)?;
    let data = cur.get_ref().get(pos..end)?;
    let mut words = Vec::with_capacity(byte_len / 2);
    for chunk in data.chunks_exact(2) {
        words.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    cur.set_position(end as u64);
    String::from_utf16(&words).ok()
}

fn skip_objd_material_list(cur: &mut Cursor<&[u8]>) -> Option<()> {
    let mut buf4 = [0u8; 4];
    cur.read_exact(&mut buf4).ok()?;
    let count = i32::from_le_bytes(buf4);
    if count < 0 {
        return None;
    }

    for _ in 0..(count as usize) {
        let mut one = [0u8; 1];
        cur.read_exact(&mut one).ok()?;
        let material_type = one[0];
        if material_type != 1 {
            cur.read_exact(&mut buf4).ok()?;
        }

        let after_rel = cur.position() + 4;
        cur.read_exact(&mut buf4).ok()?;
        let rel_end = u32::from_le_bytes(buf4) as u64;
        let material_end = after_rel.checked_add(rel_end)?;

        let mut buf2 = [0u8; 2];
        cur.read_exact(&mut buf2).ok()?;
        cur.read_exact(&mut buf4).ok()?;
        cur.read_exact(&mut buf4).ok()?;

        if material_end < cur.position() {
            return None;
        }
        cur.set_position(material_end);
        cur.read_exact(&mut buf4).ok()?;
    }

    Some(())
}

fn objd_common_name(data: &[u8]) -> Option<String> {
    let mut cur = Cursor::new(data);
    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];

    cur.read_exact(&mut buf4).ok()?;
    let version = u32::from_le_bytes(buf4);
    cur.read_exact(&mut buf4).ok()?;
    cur.read_exact(&mut buf4).ok()?;

    skip_objd_material_list(&mut cur)?;

    if version >= 0x16 {
        let _ = read_be_unicode_string(&mut cur)?;
    }

    cur.read_exact(&mut buf4).ok()?; // common version
    cur.read_exact(&mut buf8).ok()?; // name guid
    cur.read_exact(&mut buf8).ok()?; // desc guid
    read_be_unicode_string(&mut cur)
}

fn objd_catalog_name(data: &[u8]) -> Option<String> {
    let raw = objd_common_name(data)?;
    let trimmed = raw.trim();
    let value = trimmed
        .rsplit_once("Name:")
        .map(|(_, value)| value)
        .unwrap_or(trimmed)
        .trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn resource_label(entry: &DBPFEntry<'_>, name_map: &BTreeMap<u64, String>) -> String {
    if entry.resource_type == TYPE_OBJD {
        if let Some(name) = objd_catalog_name(entry.data()) {
            return name;
        }
    }

    name_map
        .get(&entry.instance)
        .cloned()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "{} {:016X}",
                kind_from_type(entry.resource_type),
                entry.instance
            )
        })
}

fn extract_xml_value_attr(text: &str, key: &str) -> Option<String> {
    let needle = format!("key=\"{}\" value=\"", key);
    let start = text.find(&needle)? + needle.len();
    let tail = &text[start..];
    let end = tail.find('"')?;
    let value = tail[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn objk_script_class_name(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data);
    let marker = "Sims3.Gameplay.Objects.";
    let start = text.find(marker)? + marker.len();
    let tail = &text[start..];
    let end = tail
        .find(|c: char| c == '\0' || c == '\r' || c == '\n' || c == '"' || c == '<' || c == ' ')
        .unwrap_or(tail.len());
    let value = tail[..end].rsplit('.').next()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn object_display_label(
    package: &DBPF<'_>,
    entry: &DBPFEntry<'_>,
    name_map: &BTreeMap<u64, String>,
) -> String {
    if entry.resource_type == TYPE_OBJD {
        if let Some(name) = objd_catalog_name(entry.data()) {
            return name;
        }

        if let Some(xml) = package.files.iter().find(|candidate| {
            candidate.resource_type == TYPE_XML && candidate.instance == entry.instance
        }) {
            let text = String::from_utf8_lossy(xml.data());
            if let Some(name) = extract_xml_value_attr(&text, "daeFileName") {
                return name;
            }
        }

        if let Some(objk) = package.files.iter().find(|candidate| {
            candidate.resource_type == TYPE_OBJK && candidate.instance == entry.instance
        }) {
            if let Some(name) = objk_script_class_name(objk.data()) {
                return name;
            }
        }
    }

    resource_label(entry, name_map)
}

fn normalized_instance_query(query: &str) -> Option<u64> {
    let lower = query.trim().to_ascii_lowercase();
    let candidate = if let Some(pos) = lower.rfind("0x") {
        &lower[pos + 2..]
    } else {
        lower.as_str()
    };
    let candidate = candidate.trim();

    if !(8..=16).contains(&candidate.len())
        || !candidate.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return None;
    }

    u64::from_str_radix(candidate, 16).ok()
}

fn item_matches_query(item: &ResourceItem, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }

    if item.label.to_lowercase().contains(&query)
        || item.key.to_lowercase().contains(&query)
        || item.kind.to_lowercase().contains(&query)
        || item.details.to_lowercase().contains(&query)
    {
        return true;
    }

    let Some(instance) = normalized_instance_query(&query) else {
        return false;
    };

    item.key.split("||").any(|part| {
        parse_key(part)
            .map(|parts| parts.instance == instance)
            .unwrap_or(false)
    })
}

fn build_item(
    package: &DBPF<'_>,
    entry: &DBPFEntry<'_>,
    name_map: &BTreeMap<u64, String>,
) -> ResourceItem {
    let kind = kind_from_type(entry.resource_type).to_string();
    let key = key_string(entry);
    let content_categories = content_categories_for_key(package, &key, name_map);
    ResourceItem {
        label: object_display_label(package, entry, name_map),
        key,
        kind,
        details: format!(
            "Type 0x{:08X} • Group 0x{:08X}",
            entry.resource_type, entry.resource_group
        ),
        content_categories,
    }
}

fn build_skintone_group_items(
    package: &DBPF<'_>,
    query: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceItem> {
    let mut groups: HashMap<String, Vec<&DBPFEntry<'_>>> = HashMap::new();

    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_SKINTONE)
    {
        let base = tone_display_name(entry, name_map);
        groups.entry(base).or_default().push(entry);
    }

    let mut items = Vec::new();
    for (base, mut entries) in groups {
        entries.sort_by_key(|entry| entry.instance);
        let first = entries[0];
        let keys: Vec<String> = entries.iter().map(|entry| key_string(entry)).collect();
        let joined_key = keys.join("||");
        let details = if entries.len() > 1 {
            format!(
                "{} arquivos TONE agrupados • Type 0x{:08X} • Group 0x{:08X}",
                entries.len(),
                first.resource_type,
                first.resource_group
            )
        } else {
            format!(
                "Type 0x{:08X} • Group 0x{:08X}",
                first.resource_type, first.resource_group
            )
        };
        let item = ResourceItem {
            label: base,
            key: joined_key,
            kind: kind_from_type(first.resource_type).to_string(),
            details,
            content_categories: vec!["Skins".to_string()],
        };
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    items
}

fn is_valid_png(data: &[u8]) -> bool {
    data.len() >= 8 && data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
}

fn encode_best_preview(entries: impl Iterator<Item = (usize, Vec<u8>)>) -> Option<String> {
    entries
        .filter_map(|(size, data)| is_valid_png(&data).then_some((size, STANDARD.encode(&data))))
        .max_by_key(|(size, _)| *size)
        .map(|(_, b64)| b64)
}

fn parse_thumbnail_triplet(value: &str) -> Option<ResourceKeyParts> {
    let mut parts = value.trim().split(':');
    let resource_type = u32::from_str_radix(parts.next()?, 16).ok()?;
    let resource_group = u32::from_str_radix(parts.next()?, 16).ok()?;
    let instance = u64::from_str_radix(parts.next()?, 16).ok()?;
    Some(ResourceKeyParts {
        resource_type,
        resource_group,
        instance,
    })
}

fn manifest_icon_ref(package: &DBPF<'_>, xml: ResourceKeyParts) -> Option<ResourceKeyParts> {
    let idx = matching_entry_index(package, xml)?;
    let text = String::from_utf8_lossy(package.files[idx].data());
    let marker = "<thumbnail>";
    let start = text.find(marker)? + marker.len();
    let end = text[start..].find("</thumbnail>")? + start;
    let parts = parse_thumbnail_triplet(text[start..end].trim())?;
    if parts.resource_type != TYPE_ICON {
        return None;
    }
    package
        .files
        .iter()
        .any(|entry| {
            entry.resource_type == parts.resource_type
                && entry.resource_group == parts.resource_group
                && entry.instance == parts.instance
        })
        .then_some(parts)
}

fn object_unique_icon_ref(package: &DBPF<'_>) -> Option<ResourceKeyParts> {
    let mut icons = package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_ICON);
    let first = icons.next()?;
    if icons.next().is_some() {
        return None;
    }
    Some(ResourceKeyParts {
        resource_type: first.resource_type,
        resource_group: first.resource_group,
        instance: first.instance,
    })
}

fn object_manifest_icon_ref(package: &DBPF<'_>, key: ResourceKeyParts) -> Option<ResourceKeyParts> {
    let object_instance = match key.resource_type {
        TYPE_OBJD | TYPE_OBJK | TYPE_VPXY | TYPE_MODL | TYPE_MLOD | TYPE_RSLT | TYPE_FTPT
        | TYPE_LITE | TYPE_RIG => key.instance,
        _ => return None,
    };

    let object_xmls: Vec<ResourceKeyParts> = package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_XML)
        .filter(|entry| entry.instance == object_instance || entry.instance == 0)
        .map(|entry| ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        })
        .collect();

    for xml in object_xmls {
        if let Some(icon) = manifest_icon_ref(package, xml) {
            return Some(icon);
        }
    }

    None
}

fn find_preview_base64(package: &DBPF<'_>, key: ResourceKeyParts) -> Option<String> {
    let thum_types = [
        TYPE_THUM_SMALL,
        TYPE_THUM_MED,
        TYPE_THUM_LARGE,
        0x0580A2B4,
        0x0580A2B5,
        0x0580A2B6,
    ];

    // 1) THUM com a mesma instance do recurso selecionado
    if let Some(b64) = encode_best_preview(
        package
            .files
            .iter()
            .filter(|entry| entry.instance == key.instance)
            .filter(|entry| {
                thum_types.contains(&entry.resource_type)
                    || (resource_is_png(entry.resource_type)
                        && matches!(
                            entry.resource_type,
                            TYPE_THUM_SMALL | TYPE_THUM_MED | TYPE_THUM_LARGE
                        ))
            })
            .map(|entry| (entry.data().len(), entry.data().to_vec())),
    ) {
        return Some(b64);
    }

    // 2) Se o próprio recurso selecionado for ICON, usar ele diretamente
    if key.resource_type == TYPE_ICON {
        if let Some(entry) = package.files.iter().find(|entry| {
            entry.resource_type == TYPE_ICON
                && entry.resource_group == key.resource_group
                && entry.instance == key.instance
        }) {
            let data = entry.data();
            if is_valid_png(data) {
                return Some(STANDARD.encode(data));
            }
        }
    }

    // 3) ICON com a mesma instance do recurso selecionado
    if let Some(b64) = encode_best_preview(
        package
            .files
            .iter()
            .filter(|entry| entry.resource_type == TYPE_ICON && entry.instance == key.instance)
            .map(|entry| (entry.data().len(), entry.data().to_vec())),
    ) {
        return Some(b64);
    }

    // 4) ICON apontado por manifest / XML de objeto
    if let Some(icon) = object_manifest_icon_ref(package, key) {
        if let Some(entry) = package.files.iter().find(|entry| {
            entry.resource_type == icon.resource_type
                && entry.resource_group == icon.resource_group
                && entry.instance == icon.instance
        }) {
            let data = entry.data();
            if is_valid_png(data) {
                return Some(STANDARD.encode(data));
            }
        }
    }

    // 5) IMG / IMAG (skin e afins)
    encode_best_preview(
        package
            .files
            .iter()
            .filter(|entry| entry.instance == key.instance)
            .filter(|entry| {
                matches!(
                    entry.resource_type,
                    TYPE_IMG | TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA
                )
            })
            .map(|entry| (entry.data().len(), entry.data().to_vec())),
    )
}

fn selected_entry_indices(package: &DBPF<'_>, keys: &HashSet<String>) -> Vec<usize> {
    let expanded = expand_compound_keys(keys);
    package
        .files
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| expanded.contains(&key_string(entry)).then_some(idx))
        .collect()
}

fn matching_entry_index(package: &DBPF<'_>, parts: ResourceKeyParts) -> Option<usize> {
    package.files.iter().position(|entry| {
        entry.resource_type == parts.resource_type
            && entry.resource_group == parts.resource_group
            && entry.instance == parts.instance
    })
}

fn export_folder_name(package_path: &str) -> String {
    Path::new(package_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn validate_package_extension(package_path: &str) -> Result<(), String> {
    let ext = Path::new(package_path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "package" | "dbc" => Ok(()),
        _ => Err("Selecione apenas arquivos .package ou .dbc.".to_string()),
    }
}

fn read_le_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_le_i32(data: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_le_u64(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn known_clone_type(resource_type: u32) -> bool {
    matches!(
        resource_type,
        TYPE_CASP
            | TYPE_FACE
            | TYPE_SKINTONE
            | TYPE_PATTERN
            | TYPE_XML
            | TYPE_THUM_SMALL
            | TYPE_THUM_MED
            | TYPE_THUM_LARGE
            | TYPE_IMG
            | TYPE_IMAG_JPG
            | TYPE_IMAG_PNG
            | TYPE_IMAG_TGA
            | TYPE_GEOM
            | TYPE_VPXY
            | TYPE_MODL
            | TYPE_KEY
            | TYPE_MLOD
            | TYPE_MTST
            | TYPE_BONE
            | TYPE_TXTC
            | TYPE_BOND
            | TYPE_BBLN
            | TYPE_BGEO
            | TYPE_FBLN
            | TYPE_OBJD
            | TYPE_OBJK
            | TYPE_RSLT
            | TYPE_FTPT
            | TYPE_LITE
            | TYPE_RIG
            | TYPE_ICON
    )
}

fn parse_trailing_tgi_block_list(data: &[u8], required: &[u32]) -> Vec<ResourceKeyParts> {
    if data.len() < 17 {
        return Vec::new();
    }

    let search_start = data.len().saturating_sub(1024);
    let mut best: Vec<ResourceKeyParts> = Vec::new();
    let mut best_offset = 0usize;

    for offset in search_start..data.len() {
        let count = data[offset] as usize;
        if count == 0 || count > 128 {
            continue;
        }
        let end = offset + 1 + count * 16;
        if end > data.len() {
            continue;
        }
        if data.len() - end > 16 {
            continue;
        }

        let mut blocks = Vec::with_capacity(count);
        let mut cursor = offset + 1;
        let mut ok = true;
        let mut hits_required = 0usize;

        for _ in 0..count {
            let instance = match read_le_u64(data, cursor) {
                Some(v) => v,
                None => {
                    ok = false;
                    break;
                }
            };
            let resource_group = match read_le_u32(data, cursor + 8) {
                Some(v) => v,
                None => {
                    ok = false;
                    break;
                }
            };
            let resource_type = match read_le_u32(data, cursor + 12) {
                Some(v) => v,
                None => {
                    ok = false;
                    break;
                }
            };
            cursor += 16;

            if !known_clone_type(resource_type) {
                ok = false;
                break;
            }
            if required.contains(&resource_type) {
                hits_required += 1;
            }
            blocks.push(ResourceKeyParts {
                resource_type,
                resource_group,
                instance,
            });
        }

        if !ok || hits_required == 0 {
            continue;
        }
        if blocks.len() > best.len() || (blocks.len() == best.len() && offset > best_offset) {
            best = blocks;
            best_offset = offset;
        }
    }

    best
}

fn tgi_refs_from_data(package: &DBPF<'_>, data: &[u8], allowed: &[u32]) -> Vec<ResourceKeyParts> {
    let mut found = HashSet::new();
    if data.len() < 16 {
        return Vec::new();
    }

    for window in data.windows(16) {
        let resource_type = u32::from_le_bytes([window[0], window[1], window[2], window[3]]);
        if !allowed.contains(&resource_type) {
            continue;
        }
        let resource_group = u32::from_le_bytes([window[4], window[5], window[6], window[7]]);
        let instance = u64::from_le_bytes([
            window[8], window[9], window[10], window[11], window[12], window[13], window[14],
            window[15],
        ]);
        let parts = ResourceKeyParts {
            resource_type,
            resource_group,
            instance,
        };
        if matching_entry_index(package, parts).is_some() {
            found.insert(parts);
        }
    }

    found.into_iter().collect()
}

fn utf16le_lossy(data: &[u8]) -> String {
    let mut words = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        words.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16_lossy(&words)
}

fn textual_key_refs_from_text(
    package: &DBPF<'_>,
    text: &str,
    allowed: &[u32],
) -> Vec<ResourceKeyParts> {
    let re = Regex::new(r"(?i)key:([0-9a-f]{8}):([0-9a-f]{8}):([0-9a-f]{16})").unwrap();
    let mut found = HashSet::new();

    for captures in re.captures_iter(text) {
        let resource_type = match u32::from_str_radix(&captures[1], 16) {
            Ok(value) if allowed.contains(&value) => value,
            _ => continue,
        };
        let resource_group = match u32::from_str_radix(&captures[2], 16) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let instance = match u64::from_str_radix(&captures[3], 16) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let parts = ResourceKeyParts {
            resource_type,
            resource_group,
            instance,
        };
        if matching_entry_index(package, parts).is_some() {
            found.insert(parts);
        }
    }

    found.into_iter().collect()
}

fn textual_key_refs_from_data(
    package: &DBPF<'_>,
    data: &[u8],
    allowed: &[u32],
) -> Vec<ResourceKeyParts> {
    let mut found = HashSet::new();

    let utf8 = String::from_utf8_lossy(data);
    for parts in textual_key_refs_from_text(package, &utf8, allowed) {
        found.insert(parts);
    }

    if data.windows(2).any(|pair| pair[1] == 0) {
        let utf16 = utf16le_lossy(data);
        for parts in textual_key_refs_from_text(package, &utf16, allowed) {
            found.insert(parts);
        }
    }

    found.into_iter().collect()
}

fn same_instance_companions(
    package: &DBPF<'_>,
    parts: ResourceKeyParts,
    include_thumbnails: bool,
) -> Vec<ResourceKeyParts> {
    let mut wanted: HashSet<u32> = HashSet::new();
    match parts.resource_type {
        TYPE_CASP => {
            if include_thumbnails {
                wanted.extend([TYPE_THUM_SMALL, TYPE_THUM_MED, TYPE_THUM_LARGE]);
            }
        }
        TYPE_TXTC => {
            wanted.extend([TYPE_IMG, TYPE_IMAG_JPG, TYPE_IMAG_PNG, TYPE_IMAG_TGA]);
        }
        TYPE_FACE => {
            wanted.extend([TYPE_BGEO]);
        }
        TYPE_VPXY => {
            wanted.extend([
                TYPE_BOND, TYPE_MTST, TYPE_XML, TYPE_RSLT, TYPE_FTPT, TYPE_LITE, TYPE_RIG,
            ]);
        }
        TYPE_MODL | TYPE_MLOD | TYPE_GEOM => {
            wanted.extend([TYPE_BONE]);
        }
        TYPE_OBJD | TYPE_OBJK => {
            wanted.extend([
                TYPE_VPXY, TYPE_XML, TYPE_TXTC, TYPE_RSLT, TYPE_FTPT, TYPE_LITE, TYPE_RIG,
            ]);
        }
        _ => {}
    }

    package
        .files
        .iter()
        .filter(|entry| entry.instance == parts.instance)
        .filter(|entry| {
            !(entry.resource_type == parts.resource_type
                && entry.resource_group == parts.resource_group)
        })
        .filter(|entry| wanted.contains(&entry.resource_type))
        .map(|entry| ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        })
        .collect()
}

fn add_key(
    resolved: &mut HashSet<String>,
    queue: &mut VecDeque<ResourceKeyParts>,
    parts: ResourceKeyParts,
) {
    let key = key_string_from_parts(parts);
    if resolved.insert(key) {
        queue.push_back(parts);
    }
}

fn enqueue_key(
    package: &DBPF<'_>,
    include_thumbnails: bool,
    resolved: &mut HashSet<String>,
    queue: &mut VecDeque<ResourceKeyParts>,
    parts: ResourceKeyParts,
) {
    add_key(resolved, queue, parts);
    for companion in same_instance_companions(package, parts, include_thumbnails) {
        add_key(resolved, queue, companion);
    }
}

fn casp_direct_refs(package: &DBPF<'_>, casp: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, casp) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    if data.len() < 9 {
        return Vec::new();
    }

    let Some(rel_tgi_pos) = read_le_i32(data, 4) else {
        return Vec::new();
    };
    if rel_tgi_pos < 0 {
        return Vec::new();
    }
    let tgi_pos = rel_tgi_pos as usize + 8;
    if tgi_pos >= data.len() {
        return Vec::new();
    }

    let count = data[tgi_pos] as usize;
    let mut cursor = tgi_pos + 1;
    let mut found = Vec::new();
    let allowed = [
        TYPE_XML,
        TYPE_BBLN,
        TYPE_VPXY,
        TYPE_TXTC,
        TYPE_BOND,
        TYPE_BGEO,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];

    for _ in 0..count {
        if cursor + 16 > data.len() {
            break;
        }
        let instance = match read_le_u64(data, cursor) {
            Some(v) => v,
            None => break,
        };
        let resource_group = match read_le_u32(data, cursor + 8) {
            Some(v) => v,
            None => break,
        };
        let resource_type = match read_le_u32(data, cursor + 12) {
            Some(v) => v,
            None => break,
        };
        cursor += 16;

        if !allowed.contains(&resource_type) {
            continue;
        }

        let parts = ResourceKeyParts {
            resource_type,
            resource_group,
            instance,
        };
        if matching_entry_index(package, parts).is_some() {
            found.push(parts);
        }
    }

    found.extend(textual_key_refs_from_data(package, data, &allowed));

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn vpxy_direct_refs(package: &DBPF<'_>, vpxy: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, vpxy) else {
        return Vec::new();
    };
    let data = package.files[idx].data();

    let mut found = Vec::new();
    let allowed = [
        TYPE_MODL, TYPE_KEY, TYPE_MLOD, TYPE_GEOM, TYPE_MTST, TYPE_XML, TYPE_TXTC, TYPE_BONE,
        TYPE_RSLT, TYPE_FTPT, TYPE_LITE, TYPE_RIG,
    ];

    // VPXY carries a TGIBlockList that explicitly points to the BONE/GEOM chain.
    // Prefer that structured list before any looser binary scan.
    for parts in parse_trailing_tgi_block_list(data, &[TYPE_BONE, TYPE_GEOM]) {
        if allowed.contains(&parts.resource_type) && matching_entry_index(package, parts).is_some()
        {
            found.push(parts);
        }
    }

    if found.is_empty() {
        found.extend(tgi_refs_from_data(package, data, &allowed));
    }
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn modl_direct_refs(package: &DBPF<'_>, modl: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, modl) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_KEY,
        TYPE_MLOD,
        TYPE_GEOM,
        TYPE_TXTC,
        TYPE_BONE,
        TYPE_XML,
        TYPE_RSLT,
        TYPE_FTPT,
        TYPE_LITE,
        TYPE_RIG,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];
    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));
    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn mlod_direct_refs(package: &DBPF<'_>, mlod: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, mlod) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_GEOM,
        TYPE_TXTC,
        TYPE_BONE,
        TYPE_XML,
        TYPE_RSLT,
        TYPE_FTPT,
        TYPE_LITE,
        TYPE_RIG,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];
    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));
    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn geom_direct_refs(package: &DBPF<'_>, geom: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, geom) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_TXTC,
        TYPE_BONE,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];
    tgi_refs_from_data(package, data, &allowed)
}

fn txtc_direct_refs(package: &DBPF<'_>, txtc: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, txtc) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_XML,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];
    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));
    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn xml_direct_refs(package: &DBPF<'_>, xml: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, xml) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_XML,
        TYPE_TXTC,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];

    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));
    if let Some(icon) = manifest_icon_ref(package, xml) {
        found.push(icon);
    }

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn bbln_direct_refs(package: &DBPF<'_>, bbln: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, bbln) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [TYPE_BGEO];
    tgi_refs_from_data(package, data, &allowed)
}

fn scan_existing_instances_from_bytes(
    package: &DBPF<'_>,
    data: &[u8],
    allowed: &[u32],
) -> Vec<ResourceKeyParts> {
    let allowed_set: HashSet<u32> = allowed.iter().copied().collect();
    let mut found = Vec::new();
    let mut seen: HashSet<ResourceKeyParts> = HashSet::new();

    for entry in package
        .files
        .iter()
        .filter(|e| allowed_set.contains(&e.resource_type))
    {
        let le = entry.instance.to_le_bytes();
        if data.windows(8).any(|w| w == le) {
            let parts = ResourceKeyParts {
                resource_type: entry.resource_type,
                resource_group: entry.resource_group,
                instance: entry.instance,
            };
            if seen.insert(parts) {
                found.push(parts);
            }
        }
    }

    found
}

fn objd_direct_refs(package: &DBPF<'_>, objd: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, objd) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_OBJK,
        TYPE_VPXY,
        TYPE_XML,
        TYPE_RSLT,
        TYPE_FTPT,
        TYPE_LITE,
        TYPE_RIG,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];

    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn objk_direct_refs(package: &DBPF<'_>, objk: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, objk) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_VPXY,
        TYPE_XML,
        TYPE_RSLT,
        TYPE_FTPT,
        TYPE_LITE,
        TYPE_RIG,
        TYPE_MODL,
        TYPE_MLOD,
        TYPE_GEOM,
        TYPE_TXTC,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];

    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn object_seed_refs(package: &DBPF<'_>, seed: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(idx) = matching_entry_index(package, seed) else {
        return Vec::new();
    };
    let data = package.files[idx].data();
    let allowed = [
        TYPE_OBJK,
        TYPE_VPXY,
        TYPE_MODL,
        TYPE_MLOD,
        TYPE_GEOM,
        TYPE_TXTC,
        TYPE_XML,
        TYPE_MTST,
        TYPE_RSLT,
        TYPE_FTPT,
        TYPE_LITE,
        TYPE_RIG,
        TYPE_IMG,
        TYPE_IMAG_JPG,
        TYPE_IMAG_PNG,
        TYPE_IMAG_TGA,
    ];

    let mut found = tgi_refs_from_data(package, data, &allowed);
    found.extend(textual_key_refs_from_data(package, data, &allowed));
    found.extend(scan_existing_instances_from_bytes(package, data, &allowed));

    let mut dedup = HashSet::new();
    found.into_iter().filter(|p| dedup.insert(*p)).collect()
}

fn object_stbl_refs(package: &DBPF<'_>, objd_instance: u64) -> Vec<ResourceKeyParts> {
    let suffix = (objd_instance & 0xFFFF_FFFF) as u32 as u64;
    package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_STBL)
        .filter(|entry| (entry.instance & 0xFFFF_FFFF) == suffix)
        .map(|entry| ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        })
        .collect()
}

fn object_group_members(package: &DBPF<'_>, objd: ResourceKeyParts) -> Vec<ResourceKeyParts> {
    let Some(entry) = package.files.iter().find(|entry| {
        entry.resource_type == objd.resource_type
            && entry.resource_group == objd.resource_group
            && entry.instance == objd.instance
    }) else {
        return Vec::new();
    };

    let mut found: HashSet<ResourceKeyParts> = HashSet::new();
    let mut vpxy_instances: HashSet<u64> = HashSet::new();
    let mut modl_instances: HashSet<u64> = HashSet::new();

    let objd_parts = ResourceKeyParts {
        resource_type: entry.resource_type,
        resource_group: entry.resource_group,
        instance: entry.instance,
    };
    found.insert(objd_parts);

    for dep in objd_direct_refs(package, objd_parts) {
        if dep.resource_type == TYPE_VPXY {
            vpxy_instances.insert(dep.instance);
        }
        if dep.resource_type == TYPE_MODL {
            modl_instances.insert(dep.instance);
        }
        found.insert(dep);

        for companion in same_instance_companions(package, dep, false) {
            if companion.resource_type == TYPE_VPXY {
                vpxy_instances.insert(companion.instance);
            }
            if companion.resource_type == TYPE_MODL {
                modl_instances.insert(companion.instance);
            }
            found.insert(companion);
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|e| e.resource_type == TYPE_OBJK && e.instance == objd.instance)
    {
        let objk_parts = ResourceKeyParts {
            resource_type: candidate.resource_type,
            resource_group: candidate.resource_group,
            instance: candidate.instance,
        };
        found.insert(objk_parts);

        for dep in objk_direct_refs(package, objk_parts) {
            if dep.resource_type == TYPE_VPXY {
                vpxy_instances.insert(dep.instance);
            }
            if dep.resource_type == TYPE_MODL {
                modl_instances.insert(dep.instance);
            }
            found.insert(dep);

            for companion in same_instance_companions(package, dep, false) {
                if companion.resource_type == TYPE_VPXY {
                    vpxy_instances.insert(companion.instance);
                }
                if companion.resource_type == TYPE_MODL {
                    modl_instances.insert(companion.instance);
                }
                found.insert(companion);
            }
        }
    }

    for candidate in package
        .files
        .iter()
        .filter(|e| e.resource_type == TYPE_VPXY && e.instance == objd.instance)
    {
        let parts = ResourceKeyParts {
            resource_type: candidate.resource_type,
            resource_group: candidate.resource_group,
            instance: candidate.instance,
        };
        found.insert(parts);
        vpxy_instances.insert(parts.instance);
    }

    let vpxy_parts: Vec<ResourceKeyParts> = package
        .files
        .iter()
        .filter(|e| e.resource_type == TYPE_VPXY && vpxy_instances.contains(&e.instance))
        .map(|e| ResourceKeyParts {
            resource_type: e.resource_type,
            resource_group: e.resource_group,
            instance: e.instance,
        })
        .collect();

    for vpxy in vpxy_parts {
        for dep in vpxy_direct_refs(package, vpxy) {
            if dep.resource_type == TYPE_MODL {
                modl_instances.insert(dep.instance);
            }
            found.insert(dep);
            for companion in same_instance_companions(package, dep, false) {
                if companion.resource_type == TYPE_MODL {
                    modl_instances.insert(companion.instance);
                }
                found.insert(companion);
            }
        }
    }

    let modl_parts: Vec<ResourceKeyParts> = package
        .files
        .iter()
        .filter(|e| e.resource_type == TYPE_MODL && modl_instances.contains(&e.instance))
        .map(|e| ResourceKeyParts {
            resource_type: e.resource_type,
            resource_group: e.resource_group,
            instance: e.instance,
        })
        .collect();

    for modl in modl_parts {
        for dep in modl_direct_refs(package, modl) {
            found.insert(dep);
            for companion in same_instance_companions(package, dep, false) {
                found.insert(companion);
            }
        }
    }

    // Final broad sweep from the object closure to catch auxiliaries that are only
    // referenced textually or through looser TGI chains in object resources.
    let seeds: Vec<ResourceKeyParts> = found.iter().copied().collect();
    for seed in seeds {
        match seed.resource_type {
            TYPE_OBJD | TYPE_OBJK | TYPE_VPXY | TYPE_MODL | TYPE_MLOD | TYPE_XML | TYPE_TXTC => {
                for dep in object_seed_refs(package, seed) {
                    found.insert(dep);
                    for companion in same_instance_companions(package, dep, false) {
                        found.insert(companion);
                    }
                }
            }
            _ => {}
        }
    }

    // Same-instance XML/TXTC companions commonly belong to the selected object closure.
    let anchor_instances: HashSet<u64> = found.iter().map(|p| p.instance).collect();
    for entry in package.files.iter().filter(|e| {
        (e.resource_type == TYPE_XML || e.resource_type == TYPE_TXTC)
            && anchor_instances.contains(&e.instance)
    }) {
        found.insert(ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        });
    }

    // Keep package/global manifest XML when present; pull ICON only if that manifest points to it.
    for entry in package
        .files
        .iter()
        .filter(|e| e.resource_type == TYPE_XML && e.instance == 0)
    {
        let parts = ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        };
        found.insert(parts);
    }

    let xml_parts: Vec<ResourceKeyParts> = found
        .iter()
        .copied()
        .filter(|p| p.resource_type == TYPE_XML)
        .collect();
    let mut found_icon = false;
    for xml in xml_parts {
        if let Some(icon) = manifest_icon_ref(package, xml) {
            found.insert(icon);
            found_icon = true;
        }
    }
    if !found_icon {
        if let Some(icon) = object_unique_icon_ref(package) {
            found.insert(icon);
        }
    }

    for part in object_stbl_refs(package, objd.instance) {
        found.insert(part);
    }

    found.into_iter().collect()
}

fn collect_clone_keys(
    package: &DBPF<'_>,
    base_keys: &HashSet<String>,
    include_thumbnails: bool,
    name_map: &BTreeMap<u64, String>,
) -> Result<HashSet<String>, String> {
    let mut resolved = HashSet::new();
    let mut queue = VecDeque::new();

    for key in base_keys {
        let parts = parse_key(key)?;
        enqueue_key(
            package,
            include_thumbnails,
            &mut resolved,
            &mut queue,
            parts,
        );
        match parts.resource_type {
            TYPE_CASP => {
                for part in casp_direct_refs(package, parts) {
                    enqueue_key(package, include_thumbnails, &mut resolved, &mut queue, part);
                }
            }
            TYPE_FACE => {
                for part in face_group_members(package, parts, name_map) {
                    enqueue_key(package, include_thumbnails, &mut resolved, &mut queue, part);
                }
            }
            TYPE_SKINTONE => {
                for part in tone_group_members(package, parts, name_map) {
                    enqueue_key(package, include_thumbnails, &mut resolved, &mut queue, part);
                }
            }
            TYPE_OBJD => {
                for part in object_group_members(package, parts) {
                    enqueue_key(package, include_thumbnails, &mut resolved, &mut queue, part);
                }
            }
            _ => {}
        }
    }

    while let Some(current) = queue.pop_front() {
        let discovered = match current.resource_type {
            TYPE_CASP => casp_direct_refs(package, current),
            TYPE_FACE => face_group_members(package, current, name_map),
            TYPE_SKINTONE => tone_group_members(package, current, name_map),
            TYPE_OBJD => object_group_members(package, current),
            TYPE_OBJK => objk_direct_refs(package, current),
            TYPE_VPXY => vpxy_direct_refs(package, current),
            TYPE_MODL => modl_direct_refs(package, current),
            TYPE_MLOD => mlod_direct_refs(package, current),
            TYPE_GEOM => geom_direct_refs(package, current),
            TYPE_TXTC => txtc_direct_refs(package, current),
            TYPE_XML => xml_direct_refs(package, current),
            TYPE_BBLN => bbln_direct_refs(package, current),
            _ => Vec::new(),
        };

        for part in discovered {
            if part == current {
                continue;
            }
            enqueue_key(package, include_thumbnails, &mut resolved, &mut queue, part);
        }
    }

    Ok(resolved)
}

fn expand_selection_for_export(
    package: &DBPF<'_>,
    base_keys: &HashSet<String>,
    include_thumbnails: bool,
    name_map: &BTreeMap<u64, String>,
) -> Result<HashSet<String>, String> {
    let mut selected = expand_selected_for_tones(package, base_keys, name_map);
    selected = expand_selected_for_faces(package, &selected, name_map);
    let extra = collect_clone_keys(package, &selected, include_thumbnails, name_map)?;
    selected.extend(extra);
    Ok(selected)
}

fn manifest_removal_plan_sets(
    package: &DBPF<'_>,
    manifest: &MergeManifest,
    selected_keys: &[String],
) -> Option<(HashSet<String>, HashSet<String>, HashSet<String>)> {
    let selected_atomic: HashSet<String> = selected_keys
        .iter()
        .flat_map(|key| key.split("||"))
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let available: HashSet<String> = package
        .files
        .iter()
        .filter(|entry| entry.resource_type != TYPE_KEY)
        .filter(|entry| !is_manifest_tgi(entry.resource_type, entry.resource_group, entry.instance))
        .map(key_string)
        .collect();

    let mut selected_packages = HashSet::new();
    for source in &manifest.packages {
        let matched = if source.anchors.is_empty() {
            source
                .resources
                .iter()
                .any(|resource| selected_atomic.contains(&resource.key))
        } else {
            source
                .anchors
                .iter()
                .any(|anchor| selected_atomic.contains(&anchor.key))
        };
        if matched {
            selected_packages.insert(source.id);
        }
    }

    if selected_packages.is_empty() {
        return None;
    }

    let mut selected_resources = HashSet::new();
    let mut remaining_resources = HashSet::new();
    for source in &manifest.packages {
        let target = if selected_packages.contains(&source.id) {
            &mut selected_resources
        } else {
            &mut remaining_resources
        };
        for resource in &source.resources {
            if available.contains(&resource.key) {
                target.insert(resource.key.clone());
            }
        }
    }

    let shared_preserved: HashSet<String> = selected_resources
        .intersection(&remaining_resources)
        .cloned()
        .collect();
    let resources_to_remove: HashSet<String> = selected_resources
        .difference(&shared_preserved)
        .cloned()
        .collect();

    Some((selected_resources, shared_preserved, resources_to_remove))
}

fn removal_plan_sets(
    package: &DBPF<'_>,
    selected_keys: &[String],
    name_map: &BTreeMap<u64, String>,
) -> Result<(HashSet<String>, HashSet<String>, HashSet<String>), String> {
    if selected_keys.is_empty() {
        return Err("Selecione pelo menos um item para remover.".to_string());
    }

    if let Some(manifest) = read_merge_manifest_from_dbpf(package)? {
        if let Some(plan) = manifest_removal_plan_sets(package, &manifest, selected_keys) {
            return Ok(plan);
        }
    }

    let selected_compound: HashSet<String> = selected_keys.iter().cloned().collect();
    let selected_resources = expand_compound_keys(&selected_compound);
    let mut candidate =
        expand_selection_for_export(package, &selected_compound, true, name_map)?;

    // KEY is metadata for the whole package and must never be removed directly.
    candidate.retain(|key| {
        parse_key(key)
            .map(|parts| parts.resource_type != TYPE_KEY)
            .unwrap_or(false)
    });

    // Treat every resource outside the candidate closure as a remaining seed.
    // Global manifest XML (instance 0) is deliberately protected even when it
    // was discovered through the selected object's dependency closure.
    let mut remaining_seeds = HashSet::new();
    for entry in &package.files {
        let key = key_string(entry);
        let global_protected = entry.resource_type == TYPE_KEY
            || (entry.resource_type == TYPE_XML && entry.instance == 0);
        if !candidate.contains(&key) || global_protected {
            remaining_seeds.insert(key);
        }
    }

    let required_by_remaining =
        collect_clone_keys(package, &remaining_seeds, true, name_map)?;
    let shared_preserved: HashSet<String> = candidate
        .intersection(&required_by_remaining)
        .cloned()
        .collect();

    let resources_to_remove: HashSet<String> = candidate
        .difference(&shared_preserved)
        .cloned()
        .collect();

    Ok((selected_resources, shared_preserved, resources_to_remove))
}

fn build_remove_selection_plan(
    package: &DBPF<'_>,
    selected_keys: &[String],
    name_map: &BTreeMap<u64, String>,
) -> Result<RemoveSelectionPlan, String> {
    let manifest_backed = read_merge_manifest_from_dbpf(package)?.is_some();
    let (selected_resources, shared_preserved, resources_to_remove) =
        removal_plan_sets(package, selected_keys, name_map)?;

    let candidate_resources = if manifest_backed {
        selected_resources.len()
    } else {
        let selected_compound: HashSet<String> = selected_keys.iter().cloned().collect();
        expand_selection_for_export(package, &selected_compound, true, name_map)?
            .into_iter()
            .filter(|key| {
                parse_key(key)
                    .map(|parts| parts.resource_type != TYPE_KEY)
                    .unwrap_or(false)
            })
            .count()
    };
    let selected_resources_preserved = selected_resources
        .intersection(&shared_preserved)
        .count();

    Ok(RemoveSelectionPlan {
        selected_items: selected_keys.len(),
        selected_resources: selected_resources.len(),
        candidate_resources,
        shared_resources_preserved: shared_preserved.len(),
        selected_resources_preserved,
        resources_to_remove: resources_to_remove.len(),
        remaining_resources: package
            .files
            .iter()
            .filter(|entry| entry.resource_type != TYPE_KEY)
            .filter(|entry| !is_manifest_tgi(entry.resource_type, entry.resource_group, entry.instance))
            .filter(|entry| !resources_to_remove.contains(&key_string(entry)))
            .count(),
    })
}

fn unique_backup_directory(destination: &Path, stem: &str) -> PathBuf {
    let base = destination.join(format!("{stem}_backup"));
    if !base.exists() {
        return base;
    }

    let mut suffix = 1usize;
    loop {
        let candidate = destination.join(format!("{stem}_backup ({suffix})"));
        if !candidate.exists() {
            return candidate;
        }
        suffix += 1;
    }
}

fn files_are_identical(left: &Path, right: &Path) -> Result<bool, String> {
    let mut a = File::open(left).map_err(|e| e.to_string())?;
    let mut b = File::open(right).map_err(|e| e.to_string())?;
    let mut a_buf = [0u8; 64 * 1024];
    let mut b_buf = [0u8; 64 * 1024];

    loop {
        let a_len = a.read(&mut a_buf).map_err(|e| e.to_string())?;
        let b_len = b.read(&mut b_buf).map_err(|e| e.to_string())?;
        if a_len != b_len {
            return Ok(false);
        }
        if a_len == 0 {
            return Ok(true);
        }
        if a_buf[..a_len] != b_buf[..b_len] {
            return Ok(false);
        }
    }
}

fn merger_anchor_kind(resource_type: u32) -> Option<&'static str> {
    match resource_type {
        TYPE_CASP => Some("CASP"),
        TYPE_OBJD => Some("OBJD"),
        TYPE_FACE => Some("FACE"),
        TYPE_SKINTONE => Some("SkinTone"),
        TYPE_PATTERN => Some("Pattern"),
        _ => None,
    }
}

fn normalized_merged_output_name(name: &str) -> String {
    let trimmed = name.trim();
    let base = if trimmed.is_empty() { "Merged.package" } else { trimmed };
    let mut safe = sanitize_file_name(base);
    if !safe.to_ascii_lowercase().ends_with(".package") {
        safe.push_str(".package");
    }
    safe
}

fn append_uncompressed_resource(
    out_pkg: &mut WritablePackage,
    out_f: &mut File,
    type_id: u32,
    group: u32,
    instance: u64,
    data: &[u8],
    unknown2: u16,
) -> Result<(), String> {
    let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
    out_f.write_all(data).map_err(|e| e.to_string())?;
    out_pkg.entries.push(WritableEntry {
        type_id,
        group,
        id1: (instance >> 32) as u32,
        id2: instance as u32,
        instance,
        chunk_offset,
        file_size: data.len() as u32,
        mem_size: data.len() as u32,
        compressed: 0,
        unknown2,
    });
    Ok(())
}

fn source_manifest_entries(
    source: &WritablePackage,
    source_path: &Path,
    next_id: &mut u32,
) -> Result<Vec<ManifestPackage>, String> {
    let available: HashSet<String> = source
        .entries
        .iter()
        .filter(|entry| entry.type_id != TYPE_KEY)
        .filter(|entry| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
        .map(key_string_from_writable)
        .collect();

    if let Some(existing) = read_merge_manifest_from_writable(source)? {
        let mut nested = Vec::new();
        let mut claimed = HashSet::new();
        for package in existing.packages {
            let resources: Vec<ManifestResource> = package
                .resources
                .into_iter()
                .filter(|resource| available.contains(&resource.key))
                .inspect(|resource| {
                    claimed.insert(resource.key.clone());
                })
                .collect();
            if resources.is_empty() {
                continue;
            }
            let anchors = package
                .anchors
                .into_iter()
                .filter(|anchor| available.contains(&anchor.key))
                .collect();
            nested.push(ManifestPackage {
                id: *next_id,
                name: package.name,
                original_size: package.original_size,
                anchors,
                resources,
            });
            *next_id += 1;
        }

        let unclaimed: Vec<String> = available.difference(&claimed).cloned().collect();
        if !unclaimed.is_empty() {
            let file_name = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("MergedSource.package")
                .to_string();
            let anchors = source
                .entries
                .iter()
                .filter_map(|entry| {
                    let kind = merger_anchor_kind(entry.type_id)?;
                    let key = key_string_from_writable(entry);
                    if !unclaimed.contains(&key) {
                        return None;
                    }
                    Some(ManifestAnchor {
                        kind: kind.to_string(),
                        key,
                    })
                })
                .collect();
            nested.push(ManifestPackage {
                id: *next_id,
                name: file_name,
                original_size: std::fs::metadata(source_path).map(|m| m.len()).unwrap_or(0),
                anchors,
                resources: unclaimed
                    .into_iter()
                    .map(|key| ManifestResource { key })
                    .collect(),
            });
            *next_id += 1;
        }
        return Ok(nested);
    }

    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Source.package")
        .to_string();
    let mut resource_keys: Vec<String> = available.into_iter().collect();
    resource_keys.sort();
    let anchors = source
        .entries
        .iter()
        .filter_map(|entry| {
            let kind = merger_anchor_kind(entry.type_id)?;
            if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance) {
                return None;
            }
            Some(ManifestAnchor {
                kind: kind.to_string(),
                key: key_string_from_writable(entry),
            })
        })
        .collect();

    let package = ManifestPackage {
        id: *next_id,
        name: file_name,
        original_size: std::fs::metadata(source_path).map(|m| m.len()).unwrap_or(0),
        anchors,
        resources: resource_keys
            .into_iter()
            .map(|key| ManifestResource { key })
            .collect(),
    };
    *next_id += 1;
    Ok(vec![package])
}

#[tauri::command]
pub fn merge_packages_with_manifest(
    payload: MergePackagesPayload,
) -> Result<MergePackagesResult, String> {
    if payload.package_paths.len() < 2 {
        return Err("Selecione pelo menos dois packages para mesclar.".to_string());
    }
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }

    let destination = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination).map_err(|e| e.to_string())?;
    let output_path = destination.join(normalized_merged_output_name(&payload.output_name));
    if output_path.exists() {
        return Err(format!(
            "Já existe um arquivo com esse nome no destino: {}",
            output_path.display()
        ));
    }

    let mut sources = Vec::new();
    let mut source_paths = Vec::new();
    for raw_path in &payload.package_paths {
        validate_package_extension(raw_path)?;
        let path = PathBuf::from(raw_path);
        let package = WritablePackage::load(&path)
            .map_err(|e| format!("Falha ao abrir {}: {e}", path.display()))?;
        sources.push(package);
        source_paths.push(path);
    }

    let mut first_owner: HashMap<String, (usize, usize)> = HashMap::new();
    let mut shared_keys = HashSet::new();
    for (source_index, source) in sources.iter().enumerate() {
        for (entry_index, entry) in source.entries.iter().enumerate() {
            if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance) {
                continue;
            }
            let key = key_string_from_writable(entry);
            if let Some((previous_source, previous_entry)) = first_owner.get(&key).copied() {
                let previous = &sources[previous_source].entries[previous_entry];
                let previous_data = sources[previous_source]
                    .get_uncompressed_data(previous)
                    .map_err(|e| e.to_string())?;
                let current_data = source
                    .get_uncompressed_data(entry)
                    .map_err(|e| e.to_string())?;
                if previous_data != current_data {
                    let left = source_paths[previous_source]
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("package");
                    let right = source_paths[source_index]
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("package");
                    return Err(format!(
                        "Conflito de TGI: {key} existe em {left} e {right} com dados diferentes. O merge foi cancelado."
                    ));
                }
                shared_keys.insert(key);
            } else {
                first_owner.insert(key, (source_index, entry_index));
            }
        }
    }

    let mut manifest_packages = Vec::new();
    let mut next_id = 1u32;
    for (source, source_path) in sources.iter().zip(source_paths.iter()) {
        manifest_packages.extend(source_manifest_entries(source, source_path, &mut next_id)?);
    }
    let manifest = MergeManifest {
        version: 1,
        packages: manifest_packages,
    };

    let mut out_pkg = WritablePackage::new(&output_path, false).map_err(|e| e.to_string())?;
    let mut out_f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&out_pkg.path)
        .map_err(|e| e.to_string())?;
    out_f
        .seek(SeekFrom::Start(out_pkg.index_position as u64))
        .map_err(|e| e.to_string())?;

    let mut written = HashSet::new();
    for (source_index, source) in sources.iter().enumerate() {
        for (entry_index, entry) in source.entries.iter().enumerate() {
            if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance) {
                continue;
            }
            let key = key_string_from_writable(entry);
            if !written.insert(key.clone()) {
                continue;
            }
            if first_owner.get(&key).copied() != Some((source_index, entry_index)) {
                continue;
            }

            let raw = source.get_raw_data(entry).map_err(|e| e.to_string())?;
            let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
            out_f.write_all(&raw).map_err(|e| e.to_string())?;
            out_pkg.entries.push(WritableEntry {
                type_id: entry.type_id,
                group: entry.group,
                id1: entry.id1,
                id2: entry.id2,
                instance: entry.instance,
                chunk_offset,
                file_size: entry.file_size,
                mem_size: entry.mem_size,
                compressed: entry.compressed,
                unknown2: entry.unknown2,
            });
        }
    }

    let manifest_bytes = manifest.to_xml_bytes();
    append_uncompressed_resource(
        &mut out_pkg,
        &mut out_f,
        MANIFEST_TYPE,
        MANIFEST_GROUP,
        MANIFEST_INSTANCE,
        &manifest_bytes,
        0,
    )?;

    out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
    out_pkg.write_index_and_header().map_err(|e| e.to_string())?;
    drop(out_f);
    drop(out_pkg);

    let rebuilt = WritablePackage::load(&output_path)
        .map_err(|e| format!("O merged final falhou na validação DBPF: {e}"))?;
    let rebuilt_manifest = read_merge_manifest_from_writable(&rebuilt)?
        .ok_or_else(|| "O merged final não contém o Merge Manifest.".to_string())?;
    let available_keys: HashSet<String> = rebuilt
        .entries
        .iter()
        .filter(|entry| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
        .map(key_string_from_writable)
        .collect();
    rebuilt_manifest.validate_keys(&available_keys)?;

    Ok(MergePackagesResult {
        output_path: output_path.display().to_string(),
        package_count: rebuilt_manifest.packages.len(),
        resource_count: written.len(),
        shared_resources: shared_keys.len(),
    })
}

#[tauri::command]
pub fn restore_manifest_packages(
    payload: RestoreManifestPayload,
) -> Result<RestoreManifestResult, String> {
    validate_package_extension(&payload.package_path)?;
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }

    let source = WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
    let manifest = read_merge_manifest_from_writable(&source)?
        .ok_or_else(|| "Este package não contém um Veiga Merge Manifest.".to_string())?;
    let destination = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination).map_err(|e| e.to_string())?;

    let index: HashMap<String, usize> = source
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
        .map(|(idx, entry)| (key_string_from_writable(entry), idx))
        .collect();

    let mut output_paths = Vec::new();
    for package_info in &manifest.packages {
        let mut file_name = sanitize_file_name(&package_info.name);
        if file_name.trim().is_empty() {
            file_name = format!("Package_{}.package", package_info.id);
        }
        if !file_name.to_ascii_lowercase().ends_with(".package") {
            file_name.push_str(".package");
        }
        let output_path = destination.join(&file_name);
        if output_path.exists() {
            return Err(format!(
                "A restauração foi cancelada porque já existe: {}",
                output_path.display()
            ));
        }

        let mut out_pkg = WritablePackage::new(&output_path, false).map_err(|e| e.to_string())?;
        let mut out_f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&out_pkg.path)
            .map_err(|e| e.to_string())?;
        out_f
            .seek(SeekFrom::Start(out_pkg.index_position as u64))
            .map_err(|e| e.to_string())?;

        let mut written = HashSet::new();
        for resource in &package_info.resources {
            if !written.insert(resource.key.clone()) {
                continue;
            }
            let entry_index = index.get(&resource.key).copied().ok_or_else(|| {
                format!(
                    "Não foi possível restaurar {}: recurso {} está ausente do merged.",
                    package_info.name, resource.key
                )
            })?;
            let entry = &source.entries[entry_index];
            let raw = source.get_raw_data(entry).map_err(|e| e.to_string())?;
            let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
            out_f.write_all(&raw).map_err(|e| e.to_string())?;
            out_pkg.entries.push(WritableEntry {
                type_id: entry.type_id,
                group: entry.group,
                id1: entry.id1,
                id2: entry.id2,
                instance: entry.instance,
                chunk_offset,
                file_size: entry.file_size,
                mem_size: entry.mem_size,
                compressed: entry.compressed,
                unknown2: entry.unknown2,
            });
        }

        out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_pkg.write_index_and_header().map_err(|e| e.to_string())?;
        drop(out_f);
        drop(out_pkg);
        WritablePackage::load(&output_path)
            .map_err(|e| format!("Package restaurado inválido ({}): {e}", file_name))?;
        output_paths.push(output_path.display().to_string());
    }

    Ok(RestoreManifestResult {
        destination_folder: destination.display().to_string(),
        package_count: output_paths.len(),
        output_paths,
    })
}

fn write_removed_package(
    package: &DBPF<'_>,
    source_package: &WritablePackage,
    source_file: &mut File,
    selected_keys: &[String],
    resources_to_remove: &HashSet<String>,
    temp_path: &Path,
) -> Result<(), String> {
    if temp_path.exists() {
        remove_file(temp_path).map_err(|e| e.to_string())?;
    }

    let retained_keys: HashSet<String> = package
        .files
        .iter()
        .filter(|entry| entry.resource_type != TYPE_KEY)
        .map(key_string)
        .filter(|key| !resources_to_remove.contains(key))
        .collect();

    let mut out_pkg = WritablePackage::new(temp_path, false).map_err(|e| e.to_string())?;
    let mut out_f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&out_pkg.path)
        .map_err(|e| e.to_string())?;
    out_f
        .seek(SeekFrom::Start(out_pkg.index_position as u64))
        .map_err(|e| e.to_string())?;

    let updated_manifest = read_merge_manifest_from_writable(source_package)?
        .map(|manifest| manifest.update_after_removal(selected_keys, resources_to_remove));

    for entry in &source_package.entries {
        let key = key_string_from_writable(entry);
        if entry.type_id == TYPE_KEY
            || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
            || resources_to_remove.contains(&key)
        {
            continue;
        }

        let raw = read_raw_writable_entry(source_file, entry)?;
        let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_f.write_all(&raw).map_err(|e| e.to_string())?;
        out_pkg.entries.push(WritableEntry {
            type_id: entry.type_id,
            group: entry.group,
            id1: entry.id1,
            id2: entry.id2,
            instance: entry.instance,
            chunk_offset,
            file_size: entry.file_size,
            mem_size: entry.mem_size,
            compressed: entry.compressed,
            unknown2: entry.unknown2,
        });
    }

    if let Some(template) = source_package
        .entries
        .iter()
        .find(|entry| entry.type_id == TYPE_KEY)
    {
        let raw = build_filtered_key_data(package, &retained_keys);
        let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_f.write_all(&raw).map_err(|e| e.to_string())?;
        out_pkg.entries.push(WritableEntry {
            type_id: template.type_id,
            group: template.group,
            id1: template.id1,
            id2: template.id2,
            instance: template.instance,
            chunk_offset,
            file_size: raw.len() as u32,
            mem_size: raw.len() as u32,
            compressed: 0,
            unknown2: template.unknown2,
        });
    }

    if let Some(manifest) = updated_manifest {
        if !manifest.packages.is_empty() {
            let data = manifest.to_xml_bytes();
            append_uncompressed_resource(
                &mut out_pkg,
                &mut out_f,
                MANIFEST_TYPE,
                MANIFEST_GROUP,
                MANIFEST_INSTANCE,
                &data,
                0,
            )?;
        }
    }

    out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
    out_pkg
        .write_index_and_header()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_removed_package(
    source_package: &WritablePackage,
    output_path: &Path,
    selected_keys: &[String],
    resources_to_remove: &HashSet<String>,
) -> Result<(), String> {
    let rebuilt = WritablePackage::load(output_path)
        .map_err(|e| format!("O package reconstruído falhou na validação DBPF: {e}"))?;

    let mut expected_counts: HashMap<String, usize> = HashMap::new();
    for entry in &source_package.entries {
        if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance) {
            continue;
        }
        let key = key_string_from_writable(entry);
        if resources_to_remove.contains(&key) {
            continue;
        }
        *expected_counts.entry(key).or_insert(0) += 1;
    }

    let mut actual_counts: HashMap<String, usize> = HashMap::new();
    for entry in &rebuilt.entries {
        if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance) {
            continue;
        }
        let key = key_string_from_writable(entry);
        if resources_to_remove.contains(&key) {
            return Err(format!(
                "Validação falhou: o recurso removido {key} ainda está no package reconstruído."
            ));
        }
        *actual_counts.entry(key).or_insert(0) += 1;
    }

    if expected_counts != actual_counts {
        return Err(
            "Validação falhou: os recursos mantidos não correspondem exatamente ao package original."
                .to_string(),
        );
    }

    let expected_manifest = read_merge_manifest_from_writable(source_package)?
        .map(|manifest| manifest.update_after_removal(selected_keys, resources_to_remove));
    let actual_manifest = read_merge_manifest_from_writable(&rebuilt)?;
    match expected_manifest {
        Some(expected) if expected.packages.is_empty() => {
            if actual_manifest.is_some() {
                return Err("Validação falhou: o Merge Manifest deveria ter sido removido.".to_string());
            }
        }
        Some(expected) => {
            let actual = actual_manifest
                .ok_or_else(|| "Validação falhou: o Merge Manifest atualizado está ausente.".to_string())?;
            if actual != expected {
                return Err("Validação falhou: o Merge Manifest atualizado não corresponde ao plano de remoção.".to_string());
            }
            let available: HashSet<String> = rebuilt
                .entries
                .iter()
                .filter(|entry| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
                .map(key_string_from_writable)
                .collect();
            actual.validate_keys(&available)?;
        }
        None => {}
    }

    let mmap = open_package(&output_path.display().to_string())?;
    let _parsed = parse_package(&mmap)?;
    Ok(())
}

fn replace_source_with_rollback(
    source_path: &Path,
    temp_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("merged.package");
    let rollback = source_path.with_file_name(format!(
        ".{file_name}.remove-original-{}",
        std::process::id()
    ));

    if rollback.exists() {
        return Err(format!(
            "Arquivo temporário de rollback já existe: {}",
            rollback.display()
        ));
    }

    rename(source_path, &rollback).map_err(|e| {
        format!(
            "Não foi possível preparar a substituição segura do original: {e}"
        )
    })?;

    if let Err(error) = rename(temp_path, output_path) {
        let _ = rename(&rollback, source_path);
        return Err(format!(
            "Não foi possível colocar o package reconstruído no destino; o original foi restaurado: {error}"
        ));
    }

    // The validated output and verified backup are already safe at this point.
    // Failure to delete the rollback copy is non-fatal; keep the extra original
    // rather than reporting the completed replacement as a failure.
    let _ = remove_file(&rollback);
    Ok(())
}

fn selection_output_stem(
    package: &DBPF<'_>,
    base_key: &str,
    name_map: &BTreeMap<u64, String>,
) -> String {
    let expanded_parts: Vec<&str> = base_key
        .split("||")
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    if expanded_parts.len() > 1 {
        let parsed: Vec<ResourceKeyParts> = expanded_parts
            .iter()
            .filter_map(|part| parse_key(part).ok())
            .collect();
        if let Some(first) = parsed.first().copied() {
            if first.resource_type == TYPE_SKINTONE {
                if let Some(entry) = package.files.iter().find(|entry| {
                    entry.resource_type == first.resource_type
                        && entry.resource_group == first.resource_group
                        && entry.instance == first.instance
                }) {
                    let label = tone_display_name(entry, name_map);
                    if !label.trim().is_empty() {
                        return sanitize_file_name(label.trim());
                    }
                }
            }
            if first.resource_type == TYPE_CASP {
                if let Some(entry) = package.files.iter().find(|entry| {
                    entry.resource_type == first.resource_type
                        && entry.resource_group == first.resource_group
                        && entry.instance == first.instance
                }) {
                    let label = object_display_label(package, entry, name_map);
                    if is_bodyhair_label(&label) {
                        return sanitize_file_name(&format!(
                            "{}-BodyHair",
                            normalized_bodyhair_name(&label)
                        ));
                    }
                }
            }
            if first.resource_type == TYPE_FACE {
                if let Some(entry) = package.files.iter().find(|entry| {
                    entry.resource_type == first.resource_type
                        && entry.resource_group == first.resource_group
                        && entry.instance == first.instance
                }) {
                    let label = normalized_face_name(&resource_label(entry, name_map));
                    if !label.trim().is_empty() {
                        return sanitize_file_name(label.trim());
                    }
                }
            }
        }
    }

    if let Ok(parts) = parse_key(first_compound_key(base_key)) {
        if let Some(entry) = package.files.iter().find(|entry| {
            entry.resource_type == parts.resource_type
                && entry.resource_group == parts.resource_group
                && entry.instance == parts.instance
        }) {
            let label = if parts.resource_type == TYPE_SKINTONE {
                tone_display_name(entry, name_map)
            } else {
                object_display_label(package, entry, name_map)
            };
            let trimmed = label.trim();
            if !trimmed.is_empty() {
                return sanitize_file_name(trimmed);
            }
        }
        return sanitize_file_name(&format!(
            "{}-{:016X}",
            kind_from_type(parts.resource_type),
            parts.instance
        ));
    }
    sanitize_file_name("resource")
}

fn clone_package_file_name(
    package: &DBPF<'_>,
    base_key: &str,
    name_map: &BTreeMap<u64, String>,
) -> String {
    format!("{}.package", selection_output_stem(package, base_key, name_map))
}

fn selected_label_file_name(
    package: &DBPF<'_>,
    selected_keys: &HashSet<String>,
    entry: &DBPFEntry<'_>,
    name_map: &BTreeMap<u64, String>,
) -> String {
    let Some(base_key) = selected_keys.iter().next() else {
        return export_file_name(entry, name_map);
    };

    let Ok(first) = parse_key(first_compound_key(base_key)) else {
        return export_file_name(entry, name_map);
    };

    if first.resource_type == TYPE_FACE {
        return export_file_name(entry, name_map);
    }

    let base_parts = expand_compound_keys(selected_keys);
    if !base_parts.contains(&key_string(entry)) {
        return export_file_name(entry, name_map);
    }

    format!(
        "{}.{}",
        selection_output_stem(package, base_key, name_map),
        extension_from_type(entry.resource_type)
    )
}

fn unique_output_name(file_name: String, used: &mut HashSet<String>) -> String {
    if used.insert(file_name.to_lowercase()) {
        return file_name;
    }

    let path = Path::new(&file_name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| file_name.clone());
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_string());

    let mut suffix = 1usize;
    loop {
        let candidate = match &ext {
            Some(ext) if !ext.is_empty() => format!("{} ({}).{}", stem, suffix, ext),
            _ => format!("{} ({})", stem, suffix),
        };
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn unique_directory_path(path: PathBuf, used: &mut HashSet<String>) -> PathBuf {
    let key = path.to_string_lossy().to_lowercase();
    if used.insert(key) {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "resource".to_string());

    let mut suffix = 1usize;
    loop {
        let candidate = parent.join(format!("{} ({})", name, suffix));
        if used.insert(candidate.to_string_lossy().to_lowercase()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn read_raw_writable_entry(
    source_file: &mut File,
    entry: &WritableEntry,
) -> Result<Vec<u8>, String> {
    source_file
        .seek(SeekFrom::Start(entry.chunk_offset as u64))
        .map_err(|e| e.to_string())?;
    let mut raw = vec![0u8; entry.file_size as usize];
    source_file.read_exact(&mut raw).map_err(|e| e.to_string())?;
    Ok(raw)
}

fn writable_entry_index(source_package: &WritablePackage) -> HashMap<String, usize> {
    let mut index = HashMap::with_capacity(source_package.entries.len());
    for (position, entry) in source_package.entries.iter().enumerate() {
        index.entry(key_string_from_writable(entry)).or_insert(position);
    }
    index
}

fn read_uncompressed_writable_entry(
    source_file: &mut File,
    entry: &WritableEntry,
) -> Result<Vec<u8>, String> {
    let raw = read_raw_writable_entry(source_file, entry)?;
    if entry.file_size == entry.mem_size {
        return Ok(raw);
    }

    sims3_pack::compression::uncompress_stream(
        &raw,
        entry.file_size as usize,
        entry.mem_size as usize,
    )
    .map_err(|e| e.to_string())
}

fn create_clone_package_from_loaded(
    core_package: &DBPF<'_>,
    name_map: &BTreeMap<u64, String>,
    source_package: &WritablePackage,
    source_file: &mut File,
    selected_keys: &[String],
    include_thumbnails: bool,
    enable_compression: bool,
    clone_path: &Path,
) -> Result<(String, usize), String> {
    let selected: HashSet<String> = selected_keys.iter().cloned().collect();
    let clone_keys =
        expand_selection_for_export(core_package, &selected, include_thumbnails, name_map)?;

    if let Some(parent) = clone_path.parent() {
        create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut out_pkg =
        WritablePackage::new(clone_path, enable_compression).map_err(|e| e.to_string())?;
    let mut out_f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&out_pkg.path)
        .map_err(|e| e.to_string())?;
    out_f
        .seek(SeekFrom::Start(out_pkg.index_position as u64))
        .map_err(|e| e.to_string())?;

    let filtered_key_data = if has_type_key(&clone_keys) {
        Some(build_filtered_key_data(core_package, &clone_keys))
    } else {
        None
    };

    let mut count = 0usize;
    for entry in &source_package.entries {
        let key = key_string_from_writable(entry);
        if !clone_keys.contains(&key) || entry.type_id == TYPE_KEY {
            continue;
        }

        let raw = read_raw_writable_entry(source_file, entry)?;
        let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_f.write_all(&raw).map_err(|e| e.to_string())?;
        out_pkg.entries.push(WritableEntry {
            type_id: entry.type_id,
            group: entry.group,
            id1: entry.id1,
            id2: entry.id2,
            instance: entry.instance,
            chunk_offset,
            file_size: entry.file_size,
            mem_size: entry.mem_size,
            compressed: entry.compressed,
            unknown2: entry.unknown2,
        });
        count += 1;
    }

    if let Some(raw) = filtered_key_data.filter(|raw| !raw.is_empty()) {
        if let Some(template) = source_package
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_KEY)
        {
            let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
            out_f.write_all(&raw).map_err(|e| e.to_string())?;
            out_pkg.entries.push(WritableEntry {
                type_id: template.type_id,
                group: template.group,
                id1: template.id1,
                id2: template.id2,
                instance: template.instance,
                chunk_offset,
                file_size: raw.len() as u32,
                mem_size: raw.len() as u32,
                compressed: 0,
                unknown2: template.unknown2,
            });
            count += 1;
        }
    }

    out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
    out_pkg
        .write_index_and_header()
        .map_err(|e| e.to_string())?;

    Ok((clone_path.display().to_string(), count))
}

fn export_selected_resources_from_loaded(
    package: &DBPF<'_>,
    name_map: &BTreeMap<u64, String>,
    source_package: &WritablePackage,
    source_file: &mut File,
    source_index: &HashMap<String, usize>,
    selected_keys: &[String],
    include_thumbnails: bool,
    export_root: &Path,
) -> Result<String, String> {
    let selected_compound: HashSet<String> = selected_keys.iter().cloned().collect();
    let selected =
        expand_selection_for_export(package, &selected_compound, include_thumbnails, name_map)?;
    create_dir_all(export_root).map_err(|e| e.to_string())?;

    let filtered_key_data = if has_type_key(&selected) {
        Some(build_filtered_key_data(package, &selected))
    } else {
        None
    };

    let mut seen_names = HashSet::new();
    for idx in selected_entry_indices(package, &selected) {
        let entry = &package.files[idx];
        if entry.resource_type == TYPE_KEY {
            continue;
        }

        let file_name = unique_output_name(
            selected_label_file_name(package, &selected_compound, entry, name_map),
            &mut seen_names,
        );
        let key = key_string(entry);
        let source_entry = source_index
            .get(&key)
            .and_then(|idx| source_package.entries.get(*idx))
            .ok_or_else(|| format!("Resource {key} was not found in the source package index."))?;
        let data = read_uncompressed_writable_entry(source_file, source_entry)?;
        std::fs::write(export_root.join(file_name), data).map_err(|e| e.to_string())?;
    }

    if let Some(raw) = filtered_key_data.filter(|raw| !raw.is_empty()) {
        if let Some(entry) = package
            .files
            .iter()
            .find(|entry| entry.resource_type == TYPE_KEY)
        {
            let file_name = unique_output_name(export_file_name(entry, name_map), &mut seen_names);
            std::fs::write(export_root.join(file_name), raw).map_err(|e| e.to_string())?;
        }
    }

    Ok(export_root.display().to_string())
}

fn zip_clone_from_loaded(
    core_package: &DBPF<'_>,
    name_map: &BTreeMap<u64, String>,
    source_package: &WritablePackage,
    source_file: &mut File,
    selected_keys: &[String],
    include_thumbnails: bool,
    enable_compression: bool,
    zip_path: &Path,
) -> Result<String, String> {
    let temp_clone = zip_path.with_extension("package");
    let (clone_path, _count) = create_clone_package_from_loaded(
        core_package,
        name_map,
        source_package,
        source_file,
        selected_keys,
        include_thumbnails,
        enable_compression,
        &temp_clone,
    )?;
    let clone_pathbuf = PathBuf::from(&clone_path);
    let zip_file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let file_name = clone_pathbuf
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    zip.start_file(file_name, options)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&clone_pathbuf).map_err(|e| e.to_string())?;
    zip.write_all(&bytes).map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&clone_pathbuf);

    Ok(zip_path.display().to_string())
}

fn zip_raw_from_loaded(
    package: &DBPF<'_>,
    name_map: &BTreeMap<u64, String>,
    source_package: &WritablePackage,
    source_file: &mut File,
    source_index: &HashMap<String, usize>,
    selected_keys: &[String],
    include_thumbnails: bool,
    zip_path: &Path,
) -> Result<String, String> {
    let selected_compound: HashSet<String> = selected_keys.iter().cloned().collect();
    let selected =
        expand_selection_for_export(package, &selected_compound, include_thumbnails, name_map)?;

    let zip_file = File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let filtered_key_data = if has_type_key(&selected) {
        Some(build_filtered_key_data(package, &selected))
    } else {
        None
    };

    let mut seen_names = HashSet::new();
    for idx in selected_entry_indices(package, &selected) {
        let entry = &package.files[idx];
        if entry.resource_type == TYPE_KEY {
            continue;
        }

        let file_name = unique_output_name(
            selected_label_file_name(package, &selected_compound, entry, name_map),
            &mut seen_names,
        );
        let key = key_string(entry);
        let source_entry = source_index
            .get(&key)
            .and_then(|idx| source_package.entries.get(*idx))
            .ok_or_else(|| format!("Resource {key} was not found in the source package index."))?;
        let data = read_uncompressed_writable_entry(source_file, source_entry)?;
        zip.start_file(file_name, options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }

    if let Some(raw) = filtered_key_data.filter(|raw| !raw.is_empty()) {
        if let Some(entry) = package
            .files
            .iter()
            .find(|entry| entry.resource_type == TYPE_KEY)
        {
            let file_name = unique_output_name(export_file_name(entry, name_map), &mut seen_names);
            zip.start_file(file_name, options)
                .map_err(|e| e.to_string())?;
            zip.write_all(&raw).map_err(|e| e.to_string())?;
        }
    }

    zip.finish().map_err(|e| e.to_string())?;
    Ok(zip_path.display().to_string())
}

fn reveal_in_file_manager(path: &Path) {
    let target = if path.exists() {
        path.to_path_buf()
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            parent.to_path_buf()
        } else {
            return;
        }
    } else {
        return;
    };

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(target).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(target).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(target).spawn();
    }
}

fn create_clone_package(payload: &ExportSelectionPayload) -> Result<(String, usize), String> {
    let destination_dir = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination_dir).map_err(|e| e.to_string())?;

    if payload.selected_keys.len() <= 1 {
        let mmap = open_package(&payload.package_path)?;
        let package = parse_package(&mmap)?;
        let name_map = package_resource_names(&package);
        let source_package =
            WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
        let mut source_file = File::open(&payload.package_path).map_err(|e| e.to_string())?;
        let clone_path = if let Some(key) = payload.selected_keys.first() {
            destination_dir.join(clone_package_file_name(&package, key, &name_map))
        } else {
            destination_dir.join(format!(
                "{}-clone.package",
                export_folder_name(&payload.package_path)
            ))
        };

        return create_clone_package_from_loaded(
            &package,
            &name_map,
            &source_package,
            &mut source_file,
            &payload.selected_keys,
            payload.include_thumbnails,
            payload.enable_compression,
            &clone_path,
        );
    }

    let batch_root = multi_export_root(&payload.destination_folder, &payload.package_path);
    create_dir_all(&batch_root).map_err(|e| e.to_string())?;

    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let name_map = package_resource_names(&package);
    let source_package = WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
    let mut source_file = File::open(&payload.package_path).map_err(|e| e.to_string())?;
    let mut total_count = 0usize;
    let mut used_clone_names = HashSet::new();

    for key in &payload.selected_keys {
        let clone_name = unique_output_name(
            clone_package_file_name(&package, key, &name_map),
            &mut used_clone_names,
        );
        let clone_path = batch_root.join(clone_name);
        let selected_keys = vec![key.clone()];
        let (_path, count) = create_clone_package_from_loaded(
            &package,
            &name_map,
            &source_package,
            &mut source_file,
            &selected_keys,
            payload.include_thumbnails,
            payload.enable_compression,
            &clone_path,
        )?;
        total_count += count;
    }

    Ok((batch_root.display().to_string(), total_count))
}

fn build_merge_manifest_items(
    package: &DBPF<'_>,
    manifest: &MergeManifest,
    query: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceItem> {
    let available: HashSet<String> = package.files.iter().map(key_string).collect();
    let mut items = Vec::new();

    for source in &manifest.packages {
        let mut selection_keys: Vec<String> = source
            .anchors
            .iter()
            .map(|anchor| anchor.key.clone())
            .filter(|key| available.contains(key))
            .collect();

        if selection_keys.is_empty() {
            selection_keys = source
                .resources
                .iter()
                .map(|resource| resource.key.clone())
                .filter(|key| available.contains(key))
                .collect();
        }
        selection_keys.sort();
        selection_keys.dedup();
        if selection_keys.is_empty() {
            continue;
        }

        let mut categories = Vec::new();
        for anchor in &source.anchors {
            if !available.contains(&anchor.key) {
                continue;
            }
            for category in content_categories_for_key(package, &anchor.key, name_map) {
                push_unique_category(&mut categories, &category);
            }
        }

        let mut kinds: Vec<String> = source
            .anchors
            .iter()
            .map(|anchor| anchor.kind.clone())
            .collect();
        kinds.sort();
        kinds.dedup();
        let kind = if kinds.is_empty() {
            "Merged CC".to_string()
        } else {
            kinds.join(" + ")
        };

        let item = ResourceItem {
            label: source.name.clone(),
            key: selection_keys.join("||"),
            kind,
            details: format!(
                "Merge Manifest • {} recurso(s) • {} anchor(s)",
                source.resources.len(),
                source.anchors.len()
            ),
            content_categories: categories,
        };
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    items
}

#[tauri::command]
pub fn scan_package_resources(payload: ScanPayload) -> Result<ScanResult, String> {
    let package_path = payload.package_path.trim();
    if package_path.is_empty() {
        return Err("Nenhum package foi selecionado.".to_string());
    }
    validate_package_extension(package_path)?;

    let mode_raw = payload.resource_type.trim().to_lowercase();
    let resource_type = match mode_raw.as_str() {
        "" => "all".to_string(),
        "all" | "casp" | "face" | "skintone" | "pattern" | "object" => mode_raw,
        _ => "all".to_string(),
    };

    std::panic::catch_unwind(|| {
        let mmap = open_package(package_path)?;
        let package = parse_package(&mmap)?;
        let query = payload.search.unwrap_or_default().to_lowercase();
        let name_map = package_resource_names(&package);
        let merge_manifest = read_merge_manifest_from_dbpf(&package)?;
        let manifest_package_count = merge_manifest
            .as_ref()
            .map(|manifest| manifest.packages.len())
            .unwrap_or(0);

        let items = if resource_type == "all" && merge_manifest.is_some() {
            build_merge_manifest_items(
                &package,
                merge_manifest.as_ref().unwrap(),
                &query,
                &name_map,
            )
        } else if resource_type == "skintone" {
            build_skintone_group_items(&package, &query, &name_map)
        } else if resource_type == "face" {
            build_face_group_items(&package, &query, &name_map)
        } else if resource_type == "casp" {
            build_casp_group_items(&package, &query, &name_map)
        } else if resource_type == "all" {
            let mut items: Vec<ResourceItem> = package
                .files
                .iter()
                .filter(|entry| filter_matches(entry, "all"))
                .filter(|entry| entry.resource_type != TYPE_SKINTONE)
                .filter(|entry| entry.resource_type != TYPE_FACE)
                .filter(|entry| entry.resource_type != TYPE_XML)
                .filter(|entry| entry.resource_type != TYPE_CASP)
                .map(|entry| build_item(&package, entry, &name_map))
                .filter(|item| item_matches_query(item, &query))
                .collect();

            items.extend(build_casp_group_items(&package, &query, &name_map));
            items.extend(build_skintone_group_items(&package, &query, &name_map));
            items.extend(build_face_group_items(&package, &query, &name_map));
            items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            items
        } else {
            let mut items: Vec<ResourceItem> = package
                .files
                .iter()
                .filter(|entry| filter_matches(entry, &resource_type))
                .filter(|entry| !(resource_type == "casp" && entry.resource_type == TYPE_XML))
                .map(|entry| build_item(&package, entry, &name_map))
                .filter(|item| item_matches_query(item, &query))
                .collect();

            items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
            items
        };

        Ok(ScanResult { items, manifest_package_count })
    })
    .map_err(|_| "Falha ao carregar a lista de recursos deste package.".to_string())?
}

#[tauri::command]
pub fn analyze_remove_selection(
    payload: RemoveSelectionPayload,
) -> Result<RemoveSelectionPlan, String> {
    validate_package_extension(&payload.package_path)?;
    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let name_map = package_resource_names(&package);
    build_remove_selection_plan(&package, &payload.selected_keys, &name_map)
}

#[tauri::command]
pub fn remove_selected_from_merged(
    payload: RemoveSelectionApplyPayload,
) -> Result<RemoveSelectionResult, String> {
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }
    validate_package_extension(&payload.package_path)?;

    let source_path = PathBuf::from(&payload.package_path);
    let destination = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination).map_err(|e| e.to_string())?;

    let file_name = source_path
        .file_name()
        .ok_or_else(|| "Nome do package de origem inválido.".to_string())?;
    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Merged");
    let output_path = destination.join(file_name);

    let source_canonical = source_path
        .canonicalize()
        .map_err(|e| format!("Não foi possível localizar o package original: {e}"))?;
    let output_is_source = output_path
        .canonicalize()
        .map(|path| path == source_canonical)
        .unwrap_or(false);

    if output_path.exists() && !output_is_source {
        return Err(format!(
            "Já existe um arquivo com o mesmo nome no destino: {}",
            output_path.display()
        ));
    }

    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let name_map = package_resource_names(&package);
    let plan = build_remove_selection_plan(&package, &payload.selected_keys, &name_map)?;
    if plan.resources_to_remove == 0 {
        return Err(
            "Nenhum recurso pode ser removido com segurança; todos os recursos selecionados são compartilhados ou protegidos."
                .to_string(),
        );
    }

    let (_selected_resources, shared_preserved, resources_to_remove) =
        removal_plan_sets(&package, &payload.selected_keys, &name_map)?;

    let source_package = WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
    let mut source_file = File::open(&payload.package_path).map_err(|e| e.to_string())?;

    let extension = source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("package");
    let temp_path = destination.join(format!(
        ".{stem}.remove-temp-{}.{}",
        std::process::id(),
        extension
    ));

    write_removed_package(
        &package,
        &source_package,
        &mut source_file,
        &payload.selected_keys,
        &resources_to_remove,
        &temp_path,
    )?;

    if let Err(error) = validate_removed_package(
        &source_package,
        &temp_path,
        &payload.selected_keys,
        &resources_to_remove,
    ) {
        let _ = remove_file(&temp_path);
        return Err(error);
    }

    let backup_dir = unique_backup_directory(&destination, stem);
    create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup_path = backup_dir.join(file_name);
    copy(&source_path, &backup_path).map_err(|e| {
        let _ = remove_file(&temp_path);
        format!("Não foi possível criar o backup do original: {e}")
    })?;

    match files_are_identical(&source_path, &backup_path) {
        Ok(true) => {}
        Ok(false) => {
            let _ = remove_file(&temp_path);
            let _ = remove_file(&backup_path);
            return Err(
                "A verificação bit-for-bit do backup falhou. O original não foi alterado."
                    .to_string(),
            );
        }
        Err(error) => {
            let _ = remove_file(&temp_path);
            let _ = remove_file(&backup_path);
            return Err(format!(
                "Não foi possível verificar o backup bit a bit. O original não foi alterado: {error}"
            ));
        }
    }

    // Close every handle backed by the original package before replacing it.
    // On Windows an open memory map/file handle can prevent rename/delete of
    // the source even though the rebuilt output and backup have already passed
    // validation.
    drop(source_file);
    drop(source_package);
    drop(package);
    drop(mmap);

    if output_is_source {
        if let Err(error) = replace_source_with_rollback(&source_path, &temp_path, &output_path) {
            let _ = remove_file(&temp_path);
            return Err(error);
        }
    } else {
        rename(&temp_path, &output_path).map_err(|e| {
            let _ = remove_file(&temp_path);
            format!("Não foi possível finalizar o novo merged: {e}")
        })?;
    }

    Ok(RemoveSelectionResult {
        output_path: output_path.display().to_string(),
        backup_path: backup_path.display().to_string(),
        removed_resources: resources_to_remove.len(),
        shared_resources_preserved: shared_preserved.len(),
        selected_resources_preserved: plan.selected_resources_preserved,
    })
}

#[tauri::command]
pub fn get_resource_preview(payload: PreviewPayload) -> Result<PreviewResult, String> {
    validate_package_extension(&payload.package_path)?;
    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let key = parse_key(first_compound_key(&payload.resource_key))?;
    let entry = package
        .files
        .iter()
        .find(|entry| {
            entry.resource_type == key.resource_type
                && entry.resource_group == key.resource_group
                && entry.instance == key.instance
        })
        .ok_or_else(|| "Resource not found in package.".to_string())?;

    let name_map = package_resource_names(&package);

    Ok(PreviewResult {
        label: object_display_label(&package, entry, &name_map),
        key: key_string(entry),
        kind: kind_from_type(entry.resource_type).to_string(),
        details: format!(
            "Type 0x{:08X} • Group 0x{:08X}",
            entry.resource_type, entry.resource_group
        ),
        thumb_base64: find_preview_base64(&package, key),
    })
}

#[tauri::command]
pub fn export_selected_resources(payload: ExportSelectionPayload) -> Result<String, String> {
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }
    validate_package_extension(&payload.package_path)?;
    if payload.create_clone_package {
        let (path, _count) = create_clone_package(&payload)?;
        return Ok(path);
    }

    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let name_map = package_resource_names(&package);
    let source_package = WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
    let mut source_file = File::open(&payload.package_path).map_err(|e| e.to_string())?;
    let source_index = writable_entry_index(&source_package);

    if payload.selected_keys.len() <= 1 {
        let export_root = PathBuf::from(&payload.destination_folder);
        return export_selected_resources_from_loaded(
            &package,
            &name_map,
            &source_package,
            &mut source_file,
            &source_index,
            &payload.selected_keys,
            payload.include_thumbnails,
            &export_root,
        );
    }

    let destination_dir = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination_dir).map_err(|e| e.to_string())?;
    let batch_root = multi_export_root(&payload.destination_folder, &payload.package_path);
    create_dir_all(&batch_root).map_err(|e| e.to_string())?;
    let batch_root_string = batch_root.display().to_string();
    let mut used_export_roots = HashSet::new();

    for key in &payload.selected_keys {
        let export_root = unique_directory_path(
            selection_export_root(&package, &batch_root_string, key, &name_map),
            &mut used_export_roots,
        );
        let selected_keys = vec![key.clone()];
        let _ = export_selected_resources_from_loaded(
            &package,
            &name_map,
            &source_package,
            &mut source_file,
            &source_index,
            &selected_keys,
            payload.include_thumbnails,
            &export_root,
        )?;
    }

    Ok(batch_root.display().to_string())
}

#[tauri::command]
pub fn zip_selected_resources(payload: ExportSelectionPayload) -> Result<String, String> {
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }
    validate_package_extension(&payload.package_path)?;

    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    let name_map = package_resource_names(&package);
    let source_package = WritablePackage::load(&payload.package_path).map_err(|e| e.to_string())?;
    let mut source_file = File::open(&payload.package_path).map_err(|e| e.to_string())?;
    let source_index = writable_entry_index(&source_package);

    if payload.selected_keys.len() <= 1 {
        let zip_name = if let Some(base_key) = payload.selected_keys.first() {
            format!("{}.zip", selection_output_stem(&package, base_key, &name_map))
        } else {
            format!("{}.zip", export_folder_name(&payload.package_path))
        };
        let zip_path = PathBuf::from(&payload.destination_folder).join(zip_name);

        if payload.create_clone_package {
            return zip_clone_from_loaded(
                &package,
                &name_map,
                &source_package,
                &mut source_file,
                &payload.selected_keys,
                payload.include_thumbnails,
                payload.enable_compression,
                &zip_path,
            );
        }

        return zip_raw_from_loaded(
            &package,
            &name_map,
            &source_package,
            &mut source_file,
            &source_index,
            &payload.selected_keys,
            payload.include_thumbnails,
            &zip_path,
        );
    }

    let destination_dir = PathBuf::from(&payload.destination_folder);
    create_dir_all(&destination_dir).map_err(|e| e.to_string())?;
    let batch_root = multi_export_root(&payload.destination_folder, &payload.package_path);
    create_dir_all(&batch_root).map_err(|e| e.to_string())?;

    let mut used_zip_names = HashSet::new();
    for key in &payload.selected_keys {
        let zip_name = unique_output_name(
            format!("{}.zip", selection_output_stem(&package, key, &name_map)),
            &mut used_zip_names,
        );
        let zip_path = batch_root.join(zip_name);
        let selected_keys = vec![key.clone()];

        if payload.create_clone_package {
            let _ = zip_clone_from_loaded(
                &package,
                &name_map,
                &source_package,
                &mut source_file,
                &selected_keys,
                payload.include_thumbnails,
                payload.enable_compression,
                &zip_path,
            )?;
        } else {
            let _ = zip_raw_from_loaded(
                &package,
                &name_map,
                &source_package,
                &mut source_file,
                &source_index,
                &selected_keys,
                payload.include_thumbnails,
                &zip_path,
            )?;
        }
    }
    Ok(batch_root.display().to_string())
}


#[cfg(test)]
mod remove_selected_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "veiga-splitter-{label}-{}-{nonce}.package",
            std::process::id()
        ))
    }

    fn write_test_package(path: &Path, resources: &[(u32, u32, u64, Vec<u8>)]) {
        let mut package = WritablePackage::new(path, false).unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&package.path)
            .unwrap();
        file.seek(SeekFrom::Start(package.index_position as u64))
            .unwrap();

        for (resource_type, group, instance, data) in resources {
            let chunk_offset = file.stream_position().unwrap() as u32;
            file.write_all(data).unwrap();
            package.entries.push(WritableEntry {
                type_id: *resource_type,
                group: *group,
                id1: (*instance >> 32) as u32,
                id2: *instance as u32,
                instance: *instance,
                chunk_offset,
                file_size: data.len() as u32,
                mem_size: data.len() as u32,
                compressed: 0,
                unknown2: 0,
            });
        }

        package.index_position = file.stream_position().unwrap() as u32;
        drop(file);
        package.write_index_and_header().unwrap();
    }

    #[test]
    fn preserves_shared_dependency_and_rebuilds_valid_package() {
        let source = test_path("remove-source");
        let rebuilt = test_path("remove-rebuilt");

        let shared_instance = 0x1111_2222_3333_4444u64;
        let selected_xml_instance = 0xAAAA_BBBB_CCCC_D001u64;
        let remaining_xml_instance = 0xAAAA_BBBB_CCCC_D002u64;
        let shared_key_text = format!(
            "key:{TYPE_IMG:08X}:{:08X}:{shared_instance:016X}",
            0u32
        );

        write_test_package(
            &source,
            &[
                (
                    TYPE_XML,
                    0,
                    selected_xml_instance,
                    format!("<selected>{shared_key_text}</selected>").into_bytes(),
                ),
                (
                    TYPE_XML,
                    0,
                    remaining_xml_instance,
                    format!("<remaining>{shared_key_text}</remaining>").into_bytes(),
                ),
                (TYPE_IMG, 0, shared_instance, vec![1, 2, 3, 4, 5, 6]),
            ],
        );

        let mmap = open_package(&source.display().to_string()).unwrap();
        let package = parse_package(&mmap).unwrap();
        let name_map = BTreeMap::new();
        let selected_key = format!(
            "0x{TYPE_XML:08X}-0x{:08X}-0x{selected_xml_instance:016X}",
            0u32
        );

        let (_selected, shared, remove) =
            removal_plan_sets(&package, &[selected_key.clone()], &name_map).unwrap();

        let shared_img_key = format!(
            "0x{TYPE_IMG:08X}-0x{:08X}-0x{shared_instance:016X}",
            0u32
        );
        assert!(remove.contains(&selected_key));
        assert!(shared.contains(&shared_img_key));
        assert!(!remove.contains(&shared_img_key));

        let source_package = WritablePackage::load(&source).unwrap();
        let mut source_file = File::open(&source).unwrap();
        write_removed_package(
            &package,
            &source_package,
            &mut source_file,
            &[selected_key.clone()],
            &remove,
            &rebuilt,
        )
        .unwrap();
        validate_removed_package(
            &source_package,
            &rebuilt,
            &[selected_key.clone()],
            &remove,
        )
        .unwrap();

        let rebuilt_package = WritablePackage::load(&rebuilt).unwrap();
        let rebuilt_keys: HashSet<String> = rebuilt_package
            .entries
            .iter()
            .map(key_string_from_writable)
            .collect();
        assert!(!rebuilt_keys.contains(&selected_key));
        assert!(rebuilt_keys.contains(&shared_img_key));

        let remaining_key = format!(
            "0x{TYPE_XML:08X}-0x{:08X}-0x{remaining_xml_instance:016X}",
            0u32
        );
        assert!(rebuilt_keys.contains(&remaining_key));

        let _ = remove_file(&source);
        let _ = remove_file(&rebuilt);
    }

    fn test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "veiga-splitter-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn reversible_merger_writes_manifest_and_restores_original_names() {
        let root = test_dir("reversible-merge");
        let source_a = root.join("TShirtMickey.package");
        let source_b = root.join("ModernSofa.package");
        let merged_dir = root.join("merged");
        let restore_dir = root.join("restored");
        std::fs::create_dir_all(&merged_dir).unwrap();
        std::fs::create_dir_all(&restore_dir).unwrap();

        let casp_instance = 0x1111_0000_0000_0001u64;
        let objd_instance = 0x2222_0000_0000_0002u64;
        let shared_instance = 0x3333_0000_0000_0003u64;
        let shared_data = vec![9, 8, 7, 6, 5];

        write_test_package(
            &source_a,
            &[
                (TYPE_CASP, 0, casp_instance, vec![1, 2, 3]),
                (TYPE_IMG, 0, shared_instance, shared_data.clone()),
            ],
        );
        write_test_package(
            &source_b,
            &[
                (TYPE_OBJD, 0, objd_instance, vec![4, 5, 6]),
                (TYPE_IMG, 0, shared_instance, shared_data),
            ],
        );

        let result = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: merged_dir.display().to_string(),
            output_name: "MyMerged.package".to_string(),
        })
        .unwrap();

        assert_eq!(result.package_count, 2);
        assert_eq!(result.shared_resources, 1);
        assert_eq!(result.resource_count, 3);

        let merged = WritablePackage::load(&result.output_path).unwrap();
        let manifest = read_merge_manifest_from_writable(&merged).unwrap().unwrap();
        assert_eq!(
            manifest
                .packages
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TShirtMickey.package", "ModernSofa.package"]
        );
        assert_eq!(manifest.packages[0].anchors[0].kind, "CASP");
        assert_eq!(manifest.packages[1].anchors[0].kind, "OBJD");

        let scan = scan_package_resources(ScanPayload {
            package_path: result.output_path.clone(),
            resource_type: "all".to_string(),
            search: None,
        })
        .unwrap();
        assert_eq!(scan.manifest_package_count, 2);
        assert_eq!(scan.items.len(), 2);
        let labels: HashSet<String> = scan.items.iter().map(|item| item.label.clone()).collect();
        assert!(labels.contains("TShirtMickey.package"));
        assert!(labels.contains("ModernSofa.package"));

        let restored = restore_manifest_packages(RestoreManifestPayload {
            package_path: result.output_path.clone(),
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();
        assert_eq!(restored.package_count, 2);
        assert!(restore_dir.join("TShirtMickey.package").exists());
        assert!(restore_dir.join("ModernSofa.package").exists());

        let restored_a = WritablePackage::load(restore_dir.join("TShirtMickey.package")).unwrap();
        let restored_a_keys: HashSet<String> = restored_a
            .entries
            .iter()
            .map(key_string_from_writable)
            .collect();
        assert!(restored_a_keys.contains(&format!(
            "0x{TYPE_CASP:08X}-0x00000000-0x{casp_instance:016X}"
        )));
        assert!(restored_a_keys.contains(&format!(
            "0x{TYPE_IMG:08X}-0x00000000-0x{shared_instance:016X}"
        )));

        let restored_b = WritablePackage::load(restore_dir.join("ModernSofa.package")).unwrap();
        let restored_b_keys: HashSet<String> = restored_b
            .entries
            .iter()
            .map(key_string_from_writable)
            .collect();
        assert!(restored_b_keys.contains(&format!(
            "0x{TYPE_OBJD:08X}-0x00000000-0x{objd_instance:016X}"
        )));
        assert!(restored_b_keys.contains(&format!(
            "0x{TYPE_IMG:08X}-0x00000000-0x{shared_instance:016X}"
        )));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reversible_merger_rejects_conflicting_tgi_data() {
        let root = test_dir("merge-conflict");
        let source_a = root.join("A.package");
        let source_b = root.join("B.package");
        let destination = root.join("out");
        std::fs::create_dir_all(&destination).unwrap();

        let instance = 0x4444_0000_0000_0004u64;
        write_test_package(&source_a, &[(TYPE_IMG, 0, instance, vec![1, 2, 3])]);
        write_test_package(&source_b, &[(TYPE_IMG, 0, instance, vec![9, 9, 9])]);

        let error = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: destination.display().to_string(),
            output_name: "Conflict.package".to_string(),
        })
        .unwrap_err();

        assert!(error.contains("Conflito de TGI"));
        assert!(!destination.join("Conflict.package").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removing_merged_cc_updates_embedded_manifest() {
        let root = test_dir("manifest-removal");
        let source_a = root.join("A.package");
        let source_b = root.join("B.package");
        let destination = root.join("out");
        std::fs::create_dir_all(&destination).unwrap();

        let casp_a = 0x5555_0000_0000_0001u64;
        let casp_b = 0x5555_0000_0000_0002u64;
        let shared = 0x5555_0000_0000_0003u64;
        write_test_package(
            &source_a,
            &[
                (TYPE_CASP, 0, casp_a, vec![1]),
                (TYPE_IMG, 0, shared, vec![7, 7]),
            ],
        );
        write_test_package(
            &source_b,
            &[
                (TYPE_CASP, 0, casp_b, vec![2]),
                (TYPE_IMG, 0, shared, vec![7, 7]),
            ],
        );

        let merged = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: destination.display().to_string(),
            output_name: "Merged.package".to_string(),
        })
        .unwrap();

        let rebuilt = root.join("rebuilt.package");
        let mmap = open_package(&merged.output_path).unwrap();
        let dbpf = parse_package(&mmap).unwrap();
        let source_package = WritablePackage::load(&merged.output_path).unwrap();
        let mut source_file = File::open(&merged.output_path).unwrap();
        let selected_key = format!(
            "0x{TYPE_CASP:08X}-0x00000000-0x{casp_a:016X}"
        );
        let name_map = package_resource_names(&dbpf);
        let (_selected_resources, shared_resources, removed) =
            removal_plan_sets(&dbpf, &[selected_key.clone()], &name_map).unwrap();
        let shared_key = format!(
            "0x{TYPE_IMG:08X}-0x00000000-0x{shared:016X}"
        );
        assert!(removed.contains(&selected_key));
        assert!(!removed.contains(&shared_key));
        assert!(shared_resources.contains(&shared_key));

        write_removed_package(
            &dbpf,
            &source_package,
            &mut source_file,
            &[selected_key.clone()],
            &removed,
            &rebuilt,
        )
        .unwrap();
        validate_removed_package(
            &source_package,
            &rebuilt,
            &[selected_key],
            &removed,
        )
        .unwrap();

        let rebuilt_package = WritablePackage::load(&rebuilt).unwrap();
        let manifest = read_merge_manifest_from_writable(&rebuilt_package)
            .unwrap()
            .unwrap();
        assert_eq!(manifest.packages.len(), 1);
        assert_eq!(manifest.packages[0].name, "B.package");
        assert!(manifest.packages[0]
            .resources
            .iter()
            .any(|r| r.key.ends_with(&format!("{shared:016X}"))));

        let _ = std::fs::remove_dir_all(&root);
    }
}


#[cfg(test)]
mod key_name_normalization_tests {
    use super::normalized_key_resource_name;

    #[test]
    fn strips_numbered_bone_suffixes() {
        assert_eq!(
            normalized_key_resource_name("afHairLongWavyLayered_BONE0"),
            "afHairLongWavyLayered"
        );
        assert_eq!(
            normalized_key_resource_name("afHairLongWavyLayered_BONE72"),
            "afHairLongWavyLayered"
        );
    }

    #[test]
    fn preserves_meaningful_underscores() {
        assert_eq!(
            normalized_key_resource_name("Creator_Name_afHairLongWavyLayered"),
            "Creator_Name_afHairLongWavyLayered"
        );
        assert_eq!(
            normalized_key_resource_name("Creator_Name_afHairLongWavyLayered_BONE12"),
            "Creator_Name_afHairLongWavyLayered"
        );
    }

    #[test]
    fn strips_multiple_technical_suffixes() {
        assert_eq!(
            normalized_key_resource_name("afHairLongWavyLayered_BONE0_LOD1"),
            "afHairLongWavyLayered"
        );
    }

    #[test]
    fn leaves_unknown_suffixes_unchanged() {
        assert_eq!(
            normalized_key_resource_name("afHairLongWavyLayered_CUSTOM12"),
            "afHairLongWavyLayered_CUSTOM12"
        );
    }
}


#[cfg(test)]
mod content_category_tests {
    use super::{casp_content_category, object_content_category, CaspFolderInfo};

    fn casp(main: &str, sub: Option<&str>) -> CaspFolderInfo {
        CaspFolderInfo {
            main_category: main.to_string(),
            sub_category: sub.map(str::to_string),
            age_folder: String::new(),
            gender_folder: String::new(),
            use_age_folder: false,
            use_gender_folder: false,
        }
    }

    #[test]
    fn classifies_core_casp_categories_exclusively() {
        assert_eq!(casp_content_category(Some(casp("Hair", Some("Hair"))), ""), "Hair");
        assert_eq!(casp_content_category(Some(casp("Hair", Some("Beard"))), ""), "Facial Hair");
        assert_eq!(casp_content_category(Some(casp("Hair", Some("Eyebrows"))), ""), "Eyebrows");
        assert_eq!(casp_content_category(Some(casp("Hair", Some("Body Hair"))), ""), "Body Hair");
        assert_eq!(casp_content_category(Some(casp("Accessories", Some("Rings"))), ""), "Accessories");
        assert_eq!(casp_content_category(Some(casp("Clothing", None)), ""), "Clothes");
        assert_eq!(casp_content_category(None, ""), "Other CASP");
    }

    #[test]
    fn classifies_special_casp_name_fallbacks() {
        assert_eq!(casp_content_category(None, "Creator_WinterBoot"), "Shoes");
        assert_eq!(casp_content_category(None, "CustomEye"), "Eyes");
        assert_eq!(casp_content_category(None, "PetCoatLong"), "Pet Coats");
        assert_eq!(casp_content_category(None, "PetAccessoryCollar"), "Pet Accessories");
    }

    #[test]
    fn classifies_object_and_build_names() {
        assert_eq!(object_content_category("Modern Sofa"), "Comfort");
        assert_eq!(object_content_category("Wall Lamp"), "Walls");
        assert_eq!(object_content_category("Grand Window"), "Windows");
        assert_eq!(object_content_category("Unknown Thing"), "Other Objects");
    }
}
