use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::convert::TryInto;
use std::fs::{copy, create_dir_all, remove_file, rename, File, OpenOptions};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use memmap::Mmap;
use num_traits::ToPrimitive;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sims3_core::dbpf::filetypes::{resource_is_png, ResourceType};
use sims3_core::dbpf::{DBPFEntry, DBPF};
use sims3_pack::dbpf::{Package as WritablePackage, ResourceEntry as WritableEntry};
use tauri::{AppHandle, Emitter, Manager};
use zip::write::FileOptions;

use crate::tone_dependencies::parse_tone_references;
use crate::merge_manifest::{
    is_manifest_tgi, read_from_dbpf as read_merge_manifest_from_dbpf,
    read_from_writable as read_merge_manifest_from_writable, ManifestAnchor, ManifestPackage,
    ManifestPreservedResource, ManifestResource, MergeManifest, MANIFEST_GROUP, MANIFEST_INSTANCE,
    MANIFEST_TYPE,
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
const TYPE_PACKAGE_MANIFEST: u32 = 0x73E93EEB;
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
const TYPE_CLIP: u32 = 0x6B20C4F3;
const TYPE_S3SA: u32 = 0x073FAA07;

// Optional relation badges must not delay loading a very large package.
const MAX_RELATION_ANALYSIS_PACKAGE_BYTES: usize = 256 * 1024 * 1024;

static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);
static PREVIEW_CACHE: OnceLock<Mutex<Option<PreviewCacheEntry>>> = OnceLock::new();

struct PreviewCacheEntry {
    // The DBPF borrows the memory map; declaration order keeps the DBPF
    // view alive only while its backing map remains alive.
    package: DBPF<'static>,
    mmap: Arc<Mmap>,
    path: PathBuf,
    file_len: u64,
    modified: Option<SystemTime>,
    name_map: BTreeMap<u64, String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PackageProgress {
    stage: String,
    message: String,
    completed_stages: u32,
    total_stages: u32,
    percent: u32,
    elapsed_ms: u128,
    resource_count: usize,
    mapped_bytes: usize,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PackageRelationUpdate {
    package_path: String,
    relations: BTreeMap<String, ResourceRelation>,
}

fn emit_package_progress(
    app: Option<&AppHandle>,
    started: Instant,
    stage: &str,
    message: &str,
    completed_stages: u32,
    total_stages: u32,
    resource_count: usize,
    mapped_bytes: usize,
) {
    let event = PackageProgress {
        stage: stage.to_string(),
        message: message.to_string(),
        completed_stages,
        total_stages,
        percent: completed_stages.saturating_mul(100) / total_stages.max(1),
        elapsed_ms: started.elapsed().as_millis(),
        resource_count,
        mapped_bytes,
    };
    eprintln!(
        "[package-progress] stage={} percent={} resources={} mapped_bytes={} elapsed_ms={}",
        event.stage, event.percent, event.resource_count, event.mapped_bytes, event.elapsed_ms
    );
    if let Some(app) = app {
        let _ = app.emit("package-progress", event);
    }
}

fn trace_removal_stage(stage: &str, started: Instant, percent: u32) {
    // A portable GUI has no visible console. Keep a minimal stage trace under
    // %TEMP% to diagnose stalls without recording package paths or CC names.
    let path = std::env::temp_dir().join("Veigas-S3CC-Splitter-BetaFinal2.1-removal.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "pid={} elapsed_ms={} percent={} stage={}",
            std::process::id(),
            started.elapsed().as_millis(),
            percent,
            stage,
        );
    }
}

fn check_cancelled() -> Result<(), String> {
    if CANCEL_REQUESTED.load(Ordering::Acquire) {
        Err("Operação cancelada com segurança.".to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn cancel_package_operation() -> Result<(), String> {
    CANCEL_REQUESTED.store(true, Ordering::Release);
    Ok(())
}

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

#[derive(Debug, Serialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRelation {
    pub duplicate_count: usize,
    pub tgi_conflict_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub items: Vec<ResourceItem>,
    pub manifest_package_count: usize,
    pub relations: BTreeMap<String, ResourceRelation>,
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
    #[serde(default)]
    pub allow_overwrite: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CollectMergeSourcesPayload {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePackagesResult {
    pub output_path: String,
    pub backup_path: Option<String>,
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
    pub duplicate_count: usize,
    pub output_paths: Vec<String>,
    pub duplicate_paths: Vec<String>,
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
    pub manifest_packages_to_remove: usize,
    pub manifest_package_total: usize,
    pub remaining_resources: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSelectionResult {
    pub output_path: String,
    pub backup_path: String,
    pub removed_resources: usize,
    pub removed_manifest_packages: usize,
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
    let length = file
        .metadata()
        .map_err(|e| format!("Não foi possível obter o tamanho do arquivo: {e}"))?
        .len();
    if std::mem::size_of::<usize>() < 8 && length > usize::MAX as u64 {
        return Err(format!(
            "Este processo de 32 bits não consegue mapear um package de {} bytes. Use a versão x64.",
            length
        ));
    }
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| {
        if std::mem::size_of::<usize>() < 8 && length > 512 * 1024 * 1024 {
            format!(
                "Não foi possível mapear o package de {} bytes em processo de 32 bits: {}. Use a versão x64.",
                length, e
            )
        } else {
            format!("Não foi possível mapear o arquivo: {e}")
        }
    })?;

    if mmap.len() < 4 {
        return Err("Arquivo inválido: muito pequeno para ser um package DBPF.".to_string());
    }

    if &mmap[0..4] != b"DBPF" {
        return Err("Arquivo inválido: não é um package DBPF válido.".to_string());
    }

    Ok(mmap)
}

fn parse_package(mmap: &Mmap) -> Result<DBPF<'_>, String> {
    parse_package_from_bytes(&mmap[..])
}

fn parse_package_from_bytes(bytes: &[u8]) -> Result<DBPF<'_>, String> {
    DBPF::new(bytes).map_err(|e| format!("Package inválido: {e:?}"))
}

fn preview_cache() -> &'static Mutex<Option<PreviewCacheEntry>> {
    PREVIEW_CACHE.get_or_init(|| Mutex::new(None))
}

fn canonical_package_path(path: &str) -> PathBuf {
    Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path))
}

fn preview_cache_is_current(cache: &PreviewCacheEntry, path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    cache.path.as_path() == path
        && cache.file_len == metadata.len()
        && cache.modified == metadata.modified().ok()
}

fn build_preview_cache(path: &Path) -> Result<PreviewCacheEntry, String> {
    let mmap = Arc::new(open_package(&path.display().to_string())?);
    // SAFETY: DBPF is only held inside this cache entry and borrows the map.
    // The DBPF field is declared before the Arc<Mmap>, so it is dropped first.
    let bytes: &'static [u8] = unsafe { std::mem::transmute::<&[u8], &'static [u8]>(&mmap[..]) };
    let package = parse_package_from_bytes(bytes)?;
    let name_map = package_resource_names(&package);
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(PreviewCacheEntry {
        package,
        mmap,
        path: path.to_path_buf(),
        file_len: metadata.len(),
        modified: metadata.modified().ok(),
        name_map,
    })
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
        TYPE_CLIP => "CLIP",
        TYPE_S3SA => "S3SA",
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
        TYPE_CLIP => "clip",
        TYPE_S3SA => "s3sa",
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
        "clip" => entry.resource_type == TYPE_CLIP,
        _ => matches!(
            entry.resource_type,
            TYPE_CASP
                | TYPE_FACE
                | TYPE_SKINTONE
                | TYPE_PATTERN
                | TYPE_XML
                | TYPE_OBJD
                | TYPE_CLIP
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
    static TONE_RE: OnceLock<Regex> = OnceLock::new();
    let re = TONE_RE.get_or_init(|| Regex::new(r"(?i)_custom_\d+$").unwrap());
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
    } else if contains_any(&[
        "decor",
        "painting",
        "sculpture",
        "rug",
        "curtain",
        "mirror",
        "plant",
    ]) {
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
            let entry = package
                .entry_index(parts.resource_type, parts.resource_group, parts.instance)
                .and_then(|index| package.files.get(index));

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
        TYPE_CLIP => {
            if pose_list_clip_instances(package).contains(&parts.instance) {
                push_unique_category(&mut categories, "Pose");
            } else {
                push_unique_category(&mut categories, "Clip");
            }
        }
        TYPE_OBJD => {
            let label = package
                .entry_index(parts.resource_type, parts.resource_group, parts.instance)
                .and_then(|index| package.files.get(index))
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
        if let Some(entry) = package
            .entry_index(parts.resource_type, parts.resource_group, parts.instance)
            .and_then(|index| package.files.get(index))
        {
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
        if let Some(entry) = package
            .entry_index(parts.resource_type, parts.resource_group, parts.instance)
            .and_then(|index| package.files.get(index))
        {
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

fn parse_key_names_data(data: &[u8]) -> BTreeMap<u64, String> {
    let mut names = BTreeMap::new();
    if data.len() < 8 {
        return names;
    }
    let Some(count) = read_le_u32(data, 4).map(|v| v as usize) else {
        return names;
    };
    if count > 100_000 {
        return names;
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
    names
}

fn build_key_data_from_names(names: &BTreeMap<u64, String>) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(8 + names.values().map(|name| 12 + name.len()).sum::<usize>());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (instance, name) in names {
        let raw = name.as_bytes();
        out.extend_from_slice(&instance.to_le_bytes());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(raw);
    }
    out
}

fn raw_package_key_names(package: &DBPF<'_>) -> BTreeMap<u64, String> {
    let mut names = BTreeMap::new();
    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_KEY)
    {
        for (instance, name) in parse_key_names_data(entry.data()) {
            names.entry(instance).or_insert(name);
        }
    }
    names
}

fn writable_key_names(source: &WritablePackage) -> Result<BTreeMap<u64, String>, String> {
    let mut names = BTreeMap::new();
    for entry in source.entries.iter().filter(|entry| entry.type_id == TYPE_KEY) {
        let data = source.get_uncompressed_data(entry).map_err(|e| {
            format!(
                "Não foi possível ler o _KEY {}: {e}",
                key_string_from_writable(entry)
            )
        })?;
        for (instance, name) in parse_key_names_data(&data) {
            names.entry(instance).or_insert(name);
        }
    }
    Ok(names)
}

fn merged_clip_key_data(sources: &[WritablePackage]) -> Result<Option<Vec<u8>>, String> {
    let mut merged = BTreeMap::<u64, String>::new();

    for source in sources {
        let clip_instances: HashSet<u64> = source
            .entries
            .iter()
            .filter(|entry| entry.type_id == TYPE_CLIP)
            .map(|entry| entry.instance)
            .collect();
        if clip_instances.is_empty() {
            continue;
        }

        for (instance, name) in writable_key_names(source)? {
            if !clip_instances.contains(&instance) {
                continue;
            }
            if let Some(existing) = merged.get(&instance) {
                if !existing.eq_ignore_ascii_case(&name) {
                    return Err(format!(
                        "Conflito no _KEY de CLIP: a Instance 0x{instance:016X} possui os nomes '{existing}' e '{name}'."
                    ));
                }
            } else {
                merged.insert(instance, name);
            }
        }
    }

    if merged.is_empty() {
        Ok(None)
    } else {
        Ok(Some(build_key_data_from_names(&merged)))
    }
}

fn fnv64_name_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.trim().to_ascii_lowercase().bytes() {
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= byte as u64;
    }
    hash
}

fn clip_age_mask(actor: &str) -> Option<u8> {
    match actor {
        "b" => Some(0x01),
        "p" => Some(0x02),
        "c" => Some(0x03),
        "t" => Some(0x04),
        "h" => Some(0x05),
        "e" => Some(0x06),
        "ad" => Some(0x08),
        "cd" => Some(0x09),
        "al" => Some(0x0A),
        "ac" => Some(0x0D),
        "cc" => Some(0x0E),
        "ah" => Some(0x10),
        "ch" => Some(0x11),
        "ab" => Some(0x12),
        "ar" => Some(0x13),
        _ => None,
    }
}

fn clip_name_hash(value: &str) -> u64 {
    let lower = value.trim().to_ascii_lowercase();
    let Some((actors, suffix)) = lower.split_once('_') else {
        return fnv64_name_hash(&lower) & 0x7FFF_FFFF_FFFF_FFFF;
    };

    if let Some((primary, target)) = actors.split_once('2') {
        if matches!(primary, "a" | "o") && matches!(target, "a" | "o") {
            return fnv64_name_hash(&lower) & 0x7FFF_FFFF_FFFF_FFFF;
        }

        let base_primary = if primary == "o" { "o" } else { "a" };
        let base_target = if target == "o" { "o" } else { "a" };
        let base = format!("{base_primary}2{base_target}_{suffix}");
        let mut hash = fnv64_name_hash(&base) | 0x8000_0000_0000_0000;
        if let Some(mask) = clip_age_mask(primary) {
            hash ^= (mask as u64) << 56;
        }
        if let Some(mask) = clip_age_mask(target) {
            hash ^= (mask as u64) << 48;
        }
        return hash;
    }

    if matches!(actors, "a" | "o") {
        return fnv64_name_hash(&lower) & 0x7FFF_FFFF_FFFF_FFFF;
    }

    let Some(mask) = clip_age_mask(actors) else {
        return fnv64_name_hash(&lower) & 0x7FFF_FFFF_FFFF_FFFF;
    };
    let base = format!("a_{suffix}");
    (fnv64_name_hash(&base) | 0x8000_0000_0000_0000) ^ ((mask as u64) << 56)
}

fn xml_tag_values(text: &str, tag: &str) -> Vec<String> {
    let pattern = format!(
        r"(?is)<{}>\s*([^<]*?)\s*</{}>",
        regex::escape(tag),
        regex::escape(tag)
    );
    let re = Regex::new(&pattern).unwrap();
    re.captures_iter(text)
        .filter_map(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn is_pose_list_text(text: &str) -> bool {
    text.to_ascii_lowercase().contains("<poselist")
}

#[derive(Debug, Clone)]
struct PoseListXmlResource {
    source_index: usize,
    entry_index: usize,
    data: Vec<u8>,
    text: String,
    registrar_entry_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct PoseListMergePlan {
    entries_to_skip: HashSet<(usize, usize)>,
    output_keys: HashSet<String>,
    primary_xml: (usize, usize),
    primary_registrar: (usize, usize),
    combined_xml: Vec<u8>,
    preserved_resources: Vec<(usize, ManifestPreservedResource)>,
}

fn pose_entry_key(block: &str) -> String {
    xml_tag_values(block, "Key")
        .into_iter()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn combine_pose_list_xmls(
    primary_text: &str,
    additional_texts: &[&str],
) -> Result<Vec<u8>, String> {
    let pose_re = Regex::new(r"(?is)<Pose\b[^>]*/>|<Pose\b[^>]*>.*?</Pose\s*>")
        .map_err(|e| format!("Regex de entradas Pose inválida: {e}"))?;
    let closing_re = Regex::new(r"(?is)</PoseList\s*>")
        .map_err(|e| format!("Regex de fechamento PoseList inválida: {e}"))?;

    let mut seen_keys = HashSet::new();
    for block in pose_re.find_iter(primary_text) {
        let key = pose_entry_key(block.as_str());
        if !key.is_empty() {
            seen_keys.insert(key.to_ascii_lowercase());
        }
    }

    let mut appended_blocks = Vec::new();
    for text in additional_texts {
        if !is_pose_list_text(text) {
            return Err("Uma das fontes selecionadas não contém um XML PoseList válido.".to_string());
        }
        for block in pose_re.find_iter(text) {
            let key = pose_entry_key(block.as_str());
            // PoseList packages commonly include one empty template entry. Keep the
            // primary template once and do not copy empty templates from later lists.
            if key.is_empty() || !seen_keys.insert(key.to_ascii_lowercase()) {
                continue;
            }
            appended_blocks.push(block.as_str().to_string());
        }
    }

    let closing = closing_re
        .find_iter(primary_text)
        .last()
        .ok_or_else(|| "O XML PoseList principal não possui fechamento </PoseList>.".to_string())?;

    let mut combined = String::with_capacity(
        primary_text.len() + appended_blocks.iter().map(String::len).sum::<usize>() + 16,
    );
    combined.push_str(&primary_text[..closing.start()]);
    if !appended_blocks.is_empty() {
        if !combined.ends_with('\n') {
            combined.push('\n');
        }
        for block in appended_blocks {
            combined.push_str(&block);
            combined.push('\n');
        }
    }
    combined.push_str(&primary_text[closing.start()..]);
    Ok(combined.into_bytes())
}

fn pose_list_merge_plan(
    sources: &[WritablePackage],
) -> Result<Option<PoseListMergePlan>, String> {
    let mut pose_lists = Vec::<PoseListXmlResource>::new();

    for (source_index, source) in sources.iter().enumerate() {
        for (entry_index, entry) in source
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.type_id == TYPE_XML)
        {
            let data = source.get_uncompressed_data(entry).map_err(|e| {
                format!(
                    "Não foi possível ler o XML {} durante a preparação do merge PoseList: {e}",
                    key_string_from_writable(entry)
                )
            })?;
            let text = String::from_utf8_lossy(&data).into_owned();
            if !is_pose_list_text(&text) {
                continue;
            }

            let registrar_entry_index = source.entries.iter().enumerate().find_map(
                |(candidate_index, candidate)| {
                    (candidate.type_id == TYPE_S3SA
                        && candidate.group == entry.group
                        && candidate.instance == entry.instance)
                        .then_some(candidate_index)
                },
            );

            pose_lists.push(PoseListXmlResource {
                source_index,
                entry_index,
                data,
                text,
                registrar_entry_index,
            });
        }
    }

    if pose_lists.len() < 2 {
        return Ok(None);
    }

    let primary_list_index = pose_lists
        .iter()
        .position(|list| list.registrar_entry_index.is_some())
        .ok_or_else(|| {
            "Foram encontrados múltiplos Pose Lists, mas nenhum possui um registrar S3SA com TGI correspondente. O merge foi cancelado para evitar um Pose Pack que não apareça no jogo.".to_string()
        })?;
    let primary_list = &pose_lists[primary_list_index];
    let primary_xml = (primary_list.source_index, primary_list.entry_index);
    let primary_registrar = (
        primary_list.source_index,
        primary_list.registrar_entry_index.unwrap(),
    );

    let additional_texts: Vec<&str> = pose_lists
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != primary_list_index)
        .map(|(_, list)| list.text.as_str())
        .collect();
    let combined_xml = combine_pose_list_xmls(&primary_list.text, &additional_texts)?;

    let mut entries_to_skip = HashSet::new();
    let mut output_keys = HashSet::new();
    let mut preserved_resources = Vec::new();
    let mut preserved_entries = HashSet::new();

    for list in &pose_lists {
        let xml_entry = &sources[list.source_index].entries[list.entry_index];
        let xml_key = key_string_from_writable(xml_entry);
        entries_to_skip.insert((list.source_index, list.entry_index));
        output_keys.insert(xml_key.clone());

        if preserved_entries.insert((list.source_index, list.entry_index)) {
            preserved_resources.push((
                list.source_index,
                ManifestPreservedResource {
                    key: xml_key,
                    data_base64: STANDARD.encode(&list.data),
                    unknown2: xml_entry.unknown2,
                },
            ));
        }

        if let Some(registrar_index) = list.registrar_entry_index {
            let registrar = &sources[list.source_index].entries[registrar_index];
            let registrar_key = key_string_from_writable(registrar);
            entries_to_skip.insert((list.source_index, registrar_index));
            output_keys.insert(registrar_key.clone());

            if preserved_entries.insert((list.source_index, registrar_index)) {
                let registrar_data = sources[list.source_index]
                    .get_uncompressed_data(registrar)
                    .map_err(|e| {
                        format!(
                            "Não foi possível preservar o registrar S3SA {}: {e}",
                            registrar_key
                        )
                    })?;
                preserved_resources.push((
                    list.source_index,
                    ManifestPreservedResource {
                        key: registrar_key,
                        data_base64: STANDARD.encode(&registrar_data),
                        unknown2: registrar.unknown2,
                    },
                ));
            }
        }
    }

    Ok(Some(PoseListMergePlan {
        entries_to_skip,
        output_keys,
        primary_xml,
        primary_registrar,
        combined_xml,
        preserved_resources,
    }))
}

fn is_pose_list_xml(entry: &DBPFEntry<'_>) -> bool {
    entry.resource_type == TYPE_XML && is_pose_list_text(&String::from_utf8_lossy(entry.data()))
}

fn pose_list_clip_hashes(text: &str) -> HashSet<u64> {
    xml_tag_values(text, "Key")
        .into_iter()
        .map(|name| clip_name_hash(&name))
        .collect()
}

fn pose_list_icon_hashes(text: &str) -> HashSet<u64> {
    xml_tag_values(text, "IconKey")
        .into_iter()
        .map(|name| fnv64_name_hash(&name))
        .collect()
}

fn pose_list_clip_instances(package: &DBPF<'_>) -> HashSet<u64> {
    let available: HashSet<u64> = package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_CLIP)
        .map(|entry| entry.instance)
        .collect();
    let mut found = HashSet::new();

    for xml in package.files.iter().filter(|entry| is_pose_list_xml(entry)) {
        let text = String::from_utf8_lossy(xml.data());
        for instance in pose_list_clip_hashes(&text) {
            if available.contains(&instance) {
                found.insert(instance);
            }
        }
    }
    found
}

fn pose_list_display_label(text: &str, instance: u64) -> String {
    if let Some(description) = xml_tag_values(text, "Description").into_iter().next() {
        return format!("Pose Pack — {description}");
    }
    if let Some(creator) = xml_tag_values(text, "Creator").into_iter().next() {
        return format!("Pose Pack — {creator}");
    }
    format!("Pose Pack 0x{instance:016X}")
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
                package.entries_for_instance(entry.instance).iter()
                    .filter_map(|&index| package.files.get(index))
                    .find_map(|candidate| {
                    if candidate.resource_type != TYPE_XML || candidate.instance != entry.instance {
                        return None;
                    }
                    let text = String::from_utf8_lossy(candidate.data());
                    extract_xml_value_attr(&text, "daeFileName")
                })
            })
            .or_else(|| {
                package.entries_for_instance(entry.instance).iter()
                    .filter_map(|&index| package.files.get(index))
                    .find_map(|candidate| {
                    if candidate.resource_type != TYPE_OBJK || candidate.instance != entry.instance
                    {
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

    build_key_data_from_names(&filtered)
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
) -> Result<Vec<ResourceKeyParts>, String> {
    let entry = package.files.iter().find(|entry| {
        entry.resource_type == tone.resource_type
            && entry.resource_group == tone.resource_group
            && entry.instance == tone.instance
    }).ok_or_else(|| format!("Selected TONE resource not found: {}", key_string_from_parts(tone)))?;

    let base = tone_display_name(entry, name_map);
    let mut found = HashSet::new();

    // Resolve exact Type/Group/Instance references from the TONE resource's
    // internal TGI block. Name and instance equality do NOT establish texture
    // ownership, especially inside third-party (s3pe) merged packages.
    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_SKINTONE)
        .filter(|candidate| tone_display_name(candidate, name_map).eq_ignore_ascii_case(&base))
    {
        found.insert(key_string(candidate));
        let references = parse_tone_references(candidate.data()).map_err(|error| {
            format!(
                "Cannot safely export skin TONE 0x{:016X}: {error}",
                candidate.instance
            )
        })?;

        // A TONE can refer to base-game textures not present in this DBPF.
        // However, when ALL its referenced _IMG resources are absent, we cannot
        // certify an independent package, as reproduced by GreatDaySimmer's
        // s3pe-merged skin exports (some contain only one TONE).
        let referenced_image_count = references.iter()
            .filter(|reference| reference.resource_type == TYPE_IMG)
            .count();
        let mut matching_image_count = 0usize;
        for reference in &references {
            if let Some(index) = package.entry_index(
                reference.resource_type,
                reference.resource_group,
                reference.instance,
            ) {
                found.insert(key_string(&package.files[index]));
                if reference.resource_type == TYPE_IMG {
                    matching_image_count += 1;
                }
            }
        }
        if referenced_image_count > 0 && matching_image_count == 0 {
            return Err(format!(
                "Skin TONE 0x{:016X} references {} _IMG entries, but none exist in the opened package. Cannot safely export an independent skin; source may rely on external resources or may already be incomplete.",
                candidate.instance, referenced_image_count
            ));
        }

        // A tone ramp's PNG swatch can share its instance with the first
        // _IMG reference but differ in Type/Group.
        if let Some(ramp) = references.first().filter(|reference| reference.resource_type == TYPE_IMG) {
            for image in package.files.iter().filter(|image| {
                image.resource_type == TYPE_IMAG_PNG && image.instance == ramp.instance
            }) {
                found.insert(key_string(image));
            }
        }
    }

    // KEY/NMAP is package-level metadata. Export code filters it to the
    // selected resource instances; do not infer a skin's type from its name.
    for candidate in package
        .files
        .iter()
        .filter(|candidate| candidate.resource_type == TYPE_KEY)
    {
        found.insert(key_string(candidate));
    }

    let mut parts = Vec::new();
    for key in found {
        if let Ok(part) = parse_key(&key) {
            parts.push(part);
        }
    }
    Ok(parts)
}

fn expand_selected_for_tones(
    package: &DBPF<'_>,
    selected: &HashSet<String>,
    name_map: &BTreeMap<u64, String>,
) -> Result<HashSet<String>, String> {
    let mut expanded = expand_compound_keys(selected);
    let base_keys: Vec<String> = expanded.iter().cloned().collect();
    for key in base_keys {
        if let Ok(parts) = parse_key(&key) {
            if parts.resource_type == TYPE_SKINTONE {
                for dep in tone_group_members(package, parts, name_map)? {
                    expanded.insert(key_string_from_parts(dep));
                }
            }
        }
    }
    Ok(expanded)
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

        if let Some(xml) = package.entries_for_instance(entry.instance).iter()
            .filter_map(|&index| package.files.get(index)).find(|candidate| {
            candidate.resource_type == TYPE_XML && candidate.instance == entry.instance
        }) {
            let text = String::from_utf8_lossy(xml.data());
            if let Some(name) = extract_xml_value_attr(&text, "daeFileName") {
                return name;
            }
        }

        if let Some(objk) = package.entries_for_instance(entry.instance).iter()
            .filter_map(|&index| package.files.get(index)).find(|candidate| {
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

    if !(8..=16).contains(&candidate.len()) || !candidate.chars().all(|ch| ch.is_ascii_hexdigit()) {
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

fn build_clip_group_items(
    package: &DBPF<'_>,
    query: &str,
    name_map: &BTreeMap<u64, String>,
) -> Vec<ResourceItem> {
    let pose_clip_instances = pose_list_clip_instances(package);
    let mut items = Vec::new();

    for xml in package.files.iter().filter(|entry| is_pose_list_xml(entry)) {
        let text = String::from_utf8_lossy(xml.data());
        let declared_clip_hashes = pose_list_clip_hashes(&text);
        let icon_hashes = pose_list_icon_hashes(&text);
        let mut keys = vec![key_string(xml)];
        let mut matched_pose_count = 0usize;

        for entry in package.files.iter().filter(|entry| entry.resource_type == TYPE_CLIP) {
            if declared_clip_hashes.contains(&entry.instance) {
                keys.push(key_string(entry));
                matched_pose_count += 1;
            }
        }

        for entry in package.files.iter().filter(|entry| {
            entry.resource_type == TYPE_S3SA && entry.instance == xml.instance
        }) {
            keys.push(key_string(entry));
        }

        for entry in package.files.iter().filter(|entry| {
            matches!(
                entry.resource_type,
                TYPE_IMG | TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA
            ) && icon_hashes.contains(&entry.instance)
        }) {
            keys.push(key_string(entry));
        }

        let xml_parts = ResourceKeyParts {
            resource_type: xml.resource_type,
            resource_group: xml.resource_group,
            instance: xml.instance,
        };
        for dep in xml_direct_refs(package, xml_parts) {
            keys.push(key_string_from_parts(dep));
        }

        keys.sort();
        keys.dedup();
        let declared_count = xml_tag_values(&text, "Key").len();
        let creator = xml_tag_values(&text, "Creator")
            .into_iter()
            .next()
            .unwrap_or_default();
        let creator_detail = if creator.is_empty() {
            String::new()
        } else {
            format!(" • {creator}")
        };

        let item = ResourceItem {
            label: pose_list_display_label(&text, xml.instance),
            key: keys.join("||"),
            kind: "Pose".to_string(),
            details: format!(
                "Pose List • {matched_pose_count}/{declared_count} pose(s) vinculada(s){creator_detail}"
            ),
            content_categories: vec!["Pose".to_string()],
        };
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == TYPE_CLIP)
        .filter(|entry| !pose_clip_instances.contains(&entry.instance))
    {
        let item = build_item(package, entry, name_map);
        if item_matches_query(&item, query) {
            items.push(item);
        }
    }

    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    items
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

// Resolve thumbnail references within this package's resource metadata,
// rather than borrowing an unrelated thumbnail or showing a stock preview.
fn linked_embedded_preview_base64(package: &DBPF<'_>, key: ResourceKeyParts) -> Option<String> {
    if !matches!(
        key.resource_type,
        TYPE_CASP | TYPE_FACE | TYPE_SKINTONE | TYPE_PATTERN | TYPE_XML
            | TYPE_OBJD | TYPE_OBJK | TYPE_VPXY | TYPE_MODL | TYPE_MLOD
            | TYPE_TXTC | TYPE_FTPT
    ) { return None; }

    let index = package.entry_index(key.resource_type, key.resource_group, key.instance)?;
    let data = package.files.get(index)?.data();
    if data.len() < 8 || data.len() > 1024 * 1024 { return None; }

    let mut seen = HashSet::new();
    let mut thumbs = Vec::new();
    let mut images = Vec::new();
    for window in data.windows(8) {
        let instance = u64::from_le_bytes(window.try_into().ok()?);
        let matches = package.entries_for_instance(instance);
        // Most sliding 8-byte windows do not identify any real resource:
        // don't allocate an entry in the set for those false candidates.
        if matches.is_empty() || !seen.insert(instance) { continue; }
        for &candidate_index in matches {
            let Some(entry) = package.files.get(candidate_index) else { continue };
            let is_thumb = matches!(
                entry.resource_type, TYPE_THUM_SMALL | TYPE_THUM_MED | TYPE_THUM_LARGE
                    | 0x0580A2B4 | 0x0580A2B5 | 0x0580A2B6 | TYPE_ICON
            );
            let is_image = matches!(entry.resource_type, TYPE_IMG | TYPE_IMAG_PNG);
            if !(is_thumb || is_image) || !is_valid_png(entry.data()) { continue; }
            let bytes = entry.data().to_vec();
            if is_thumb { thumbs.push((bytes.len(), bytes)); }
            else { images.push((bytes.len(), bytes)); }
        }
        if thumbs.len() >= 16 || images.len() >= 16 { break; }
    }
    encode_best_preview(thumbs.into_iter())
        .or_else(|| encode_best_preview(images.into_iter()))
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
        package.entries_for_instance(key.instance).iter()
            .filter_map(|&index| package.files.get(index))
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
        if let Some(entry) = package.entry_index(TYPE_ICON, key.resource_group, key.instance)
            .and_then(|index| package.files.get(index)) {
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
        if let Some(entry) = package.entry_index(icon.resource_type, icon.resource_group, icon.instance)
            .and_then(|index| package.files.get(index)) {
            let data = entry.data();
            if is_valid_png(data) {
                return Some(STANDARD.encode(data));
            }
        }
    }

    // 5) IMG / IMAG (skin e afins)
    encode_best_preview(
        package.entries_for_instance(key.instance).iter()
            .filter_map(|&index| package.files.get(index))
            .filter(|entry| {
                matches!(
                    entry.resource_type,
                    TYPE_IMG | TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA
                )
            })
            .map(|entry| (entry.data().len(), entry.data().to_vec())),
    ).or_else(|| linked_embedded_preview_base64(package, key))
}

fn sims3_thumbnails_dir(app: &AppHandle) -> Option<PathBuf> {
    let documents = app.path().document_dir().ok()?;
    let path = documents
        .join("Electronic Arts")
        .join("The Sims 3")
        .join("Thumbnails");
    path.is_dir().then_some(path)
}

fn thumbnail_cache_file_order(resource_type: u32) -> [&'static str; 3] {
    const CAS: &str = "CASThumbnails.package";
    const OBJECTS: &str = "ObjectThumbnails.package";
    const DOWNLOADS: &str = "DownloadsThumbnails.package";

    match resource_type {
        TYPE_CASP | TYPE_FACE | TYPE_SKINTONE => [CAS, OBJECTS, DOWNLOADS],
        TYPE_OBJD | TYPE_OBJK | TYPE_VPXY | TYPE_MODL | TYPE_MLOD | TYPE_RSLT | TYPE_FTPT
        | TYPE_LITE | TYPE_RIG => [OBJECTS, CAS, DOWNLOADS],
        _ => [CAS, OBJECTS, DOWNLOADS],
    }
}

fn thumbnail_from_cache_package(path: &Path, instance: u64) -> Option<String> {
    // Large Sims 3 thumbnail caches can be several gigabytes. Use a memory map
    // instead of allocating a full heap copy for the automatic first preview.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !path.is_file() {
            return None;
        }
        let file = File::open(path).ok()?;
        let mmap = unsafe { Mmap::map(&file).ok()? };
        let package = DBPF::new(&mmap[..]).ok()?;
        encode_best_preview(
            package
                .files
                .iter()
                .filter(|entry| entry.instance == instance)
                .map(|entry| (entry.data().len(), entry.data().to_vec())),
        )
    }))
    .ok()
    .flatten()
}

fn external_thumbnail_base64_from_dir(
    thumbnails_dir: &Path,
    key: ResourceKeyParts,
) -> Option<String> {
    for file_name in thumbnail_cache_file_order(key.resource_type) {
        if let Some(preview) =
            thumbnail_from_cache_package(&thumbnails_dir.join(file_name), key.instance)
        {
            return Some(preview);
        }
    }
    None
}

fn external_thumbnail_base64(app: &AppHandle, key: ResourceKeyParts) -> Option<String> {
    let thumbnails_dir = sims3_thumbnails_dir(app)?;
    external_thumbnail_base64_from_dir(&thumbnails_dir, key)
}

fn pose_list_preview_base64(package: &DBPF<'_>, text: &str) -> Option<String> {
    let icon_hashes = pose_list_icon_hashes(text);
    encode_best_preview(
        package
            .files
            .iter()
            .filter(|entry| {
                matches!(
                    entry.resource_type,
                    TYPE_IMG | TYPE_IMAG_JPG | TYPE_IMAG_PNG | TYPE_IMAG_TGA
                ) && icon_hashes.contains(&entry.instance)
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
    package.entry_index(parts.resource_type, parts.resource_group, parts.instance)
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
            | TYPE_CLIP
            | TYPE_S3SA
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
    static TEXTUAL_KEY_RE: OnceLock<Regex> = OnceLock::new();
    let re = TEXTUAL_KEY_RE.get_or_init(|| {
        Regex::new(r"(?i)key:([0-9a-f]{8}):([0-9a-f]{8}):([0-9a-f]{16})").unwrap()
    });
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

    if wanted.is_empty() {
        return Vec::new();
    }
    // The DBPF instance index avoids rescanning every file for each resource
    // in the generic removal graph (quadratic on large CC collections).
    package
        .entries_for_instance(parts.instance)
        .iter()
        .filter_map(|&index| package.files.get(index))
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
    // Preserve the original dependency-closure semantics, including cases
    // where a key was first queued as another resource's companion.
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
    if data.len() < 8 || allowed.is_empty() {
        return Vec::new();
    }

    // Previous implementation searched every byte of the payload separately
    // for each resource in the merged: O(resources * payload_length).
    // Look up each 8-byte window in the indexed DBPF instances instead.
    // Only actual matches are stored, so random binary data cannot trigger
    // an unbounded allocation.
    let mut matched_indices: HashSet<usize> = HashSet::new();
    for window in data.windows(8) {
        let instance = u64::from_le_bytes(window.try_into().expect("8-byte window"));
        for &idx in package.entries_for_instance(instance).iter() {
            if let Some(entry) = package.files.get(idx) {
                if allowed.contains(&entry.resource_type) {
                    matched_indices.insert(idx);
                }
            }
        }
    }

    // Preserve the original resource ordering and duplicate-TGI handling.
    let mut seen: HashSet<ResourceKeyParts> = HashSet::new();
    package
        .files
        .iter()
        .enumerate()
        .filter(|(idx, _)| matched_indices.contains(idx))
        .filter_map(|(_, entry)| {
            let parts = ResourceKeyParts {
                resource_type: entry.resource_type,
                resource_group: entry.resource_group,
                instance: entry.instance,
            };
            seen.insert(parts).then_some(parts)
        })
        .collect()
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
                for part in tone_group_members(package, parts, name_map)? {
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
            TYPE_SKINTONE => tone_group_members(package, current, name_map)?,
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
    let mut selected = expand_selected_for_tones(package, base_keys, name_map)?;
    selected = expand_selected_for_faces(package, &selected, name_map);
    let extra = collect_clone_keys(package, &selected, include_thumbnails, name_map)?;
    selected.extend(extra);

    // A merged DBPF can contain icons for several independent objects.
    // Their instances need not match OBJD, and copying an arbitrary icon
    // would silently attach another object's preview. On Veiga merges, the
    // embedded per-source manifest provides the exact ownership relationship.
    let requested_objects: HashSet<String> = selected
        .iter()
        .filter(|key| {
            parse_key(key)
                .map(|parts| parts.resource_type == TYPE_OBJD)
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if !requested_objects.is_empty() {
        if let Some(manifest) = read_merge_manifest_from_dbpf(package)? {
            for source in &manifest.packages {
                let owns_requested_object = source.anchors.iter().any(|anchor| {
                    anchor.kind.eq_ignore_ascii_case("OBJD")
                        && requested_objects.contains(&anchor.key)
                });
                if !owns_requested_object {
                    continue;
                }
                let matching_icons: Vec<&str> = source.resources.iter()
                    .filter_map(|resource| {
                        let parts = parse_key(&resource.key).ok()?;
                        (parts.resource_type == TYPE_ICON
                            && matching_entry_index(package, parts).is_some())
                            .then_some(resource.key.as_str())
                    })
                    .collect();
                // If an original source had multiple ICONs, resource ownership
                // inside that source is ambiguous; do not guess.
                if matching_icons.len() == 1 {
                    selected.insert(matching_icons[0].to_string());
                }
            }
        }
    }

    let key_names = raw_package_key_names(package);
    let needs_key = selected.iter().any(|key| {
        parse_key(key)
            .map(|parts| parts.resource_type == TYPE_CLIP && key_names.contains_key(&parts.instance))
            .unwrap_or(false)
    });
    if needs_key {
        for entry in package
            .files
            .iter()
            .filter(|entry| entry.resource_type == TYPE_KEY)
        {
            selected.insert(key_string(entry));
        }
    }

    Ok(selected)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct LogicalAtomKey {
    resource_type: u32,
    resource_group: u32,
    data_len: usize,
    hash_a: u64,
    hash_b: u64,
}

#[derive(Clone, Debug)]
struct LogicalAtom {
    key: LogicalAtomKey,
    entry_index: usize,
}

#[derive(Clone, Debug)]
struct LogicalProfile {
    atoms: Vec<LogicalAtom>,
}

fn relation_ignored_resource(parts: ResourceKeyParts) -> bool {
    parts.resource_type == TYPE_KEY
        || parts.resource_type == TYPE_PACKAGE_MANIFEST
        || is_manifest_tgi(parts.resource_type, parts.resource_group, parts.instance)
}

fn relation_hash_pair(data: &[u8]) -> (u64, u64) {
    let mut a = 0xcbf29ce484222325u64;
    let mut b = 0x84222325cbf29ce4u64 ^ data.len() as u64;

    for &byte in data {
        a ^= byte as u64;
        a = a.wrapping_mul(0x100000001b3);

        b ^= (byte as u64).wrapping_add(0x9e3779b97f4a7c15);
        b = b.rotate_left(13).wrapping_mul(0xff51afd7ed558ccd);
        b ^= b >> 29;
    }

    (a, b)
}

fn relation_tgi_index(package: &DBPF<'_>) -> HashMap<ResourceKeyParts, Vec<usize>> {
    let mut index: HashMap<ResourceKeyParts, Vec<usize>> = HashMap::new();

    for (entry_index, entry) in package.files.iter().enumerate() {
        let parts = ResourceKeyParts {
            resource_type: entry.resource_type,
            resource_group: entry.resource_group,
            instance: entry.instance,
        };
        index.entry(parts).or_default().push(entry_index);
    }

    index
}

fn relation_entries_have_same_payload(package: &DBPF<'_>, indices: &[usize]) -> bool {
    let Some(first_index) = indices.first().copied() else {
        return true;
    };
    let first = package.files[first_index].data();
    indices
        .iter()
        .copied()
        .skip(1)
        .all(|entry_index| package.files[entry_index].data() == first)
}

fn relation_tgi_conflicts(
    package: &DBPF<'_>,
    tgi_index: &HashMap<ResourceKeyParts, Vec<usize>>,
) -> HashMap<ResourceKeyParts, usize> {
    let mut conflicts = HashMap::new();

    for (parts, indices) in tgi_index {
        if relation_ignored_resource(*parts) || indices.len() < 2 {
            continue;
        }
        if !relation_entries_have_same_payload(package, indices) {
            conflicts.insert(*parts, indices.len());
        }
    }

    conflicts
}

fn logical_profile_for_item(
    package: &DBPF<'_>,
    item: &ResourceItem,
    name_map: &BTreeMap<u64, String>,
    tgi_index: &HashMap<ResourceKeyParts, Vec<usize>>,
    tgi_conflicts: &HashMap<ResourceKeyParts, usize>,
    atom_cache: &mut HashMap<usize, LogicalAtomKey>,
) -> Result<(Option<LogicalProfile>, usize), String> {
    let base_keys: HashSet<String> = item
        .key
        .split("||")
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if base_keys.is_empty() {
        return Ok((None, 0));
    }

    let expanded = expand_selection_for_export(package, &base_keys, false, name_map)?;
    let mut ordered_keys: Vec<String> = expanded.into_iter().collect();
    ordered_keys.sort();

    let mut atoms = Vec::new();
    let mut conflict_count = 0usize;

    for key in ordered_keys {
        let parts = parse_key(&key)?;
        if relation_ignored_resource(parts) {
            continue;
        }

        let Some(indices) = tgi_index.get(&parts) else {
            continue;
        };
        if indices.is_empty() {
            continue;
        }

        if let Some(count) = tgi_conflicts.get(&parts) {
            conflict_count = conflict_count.max(*count);
            continue;
        }

        let entry_index = indices[0];
        let atom_key = *atom_cache.entry(entry_index).or_insert_with(|| {
            let data = package.files[entry_index].data();
            let (hash_a, hash_b) = relation_hash_pair(data);
            LogicalAtomKey {
                resource_type: parts.resource_type,
                resource_group: parts.resource_group,
                data_len: data.len(),
                hash_a,
                hash_b,
            }
        });
        atoms.push(LogicalAtom {
            key: atom_key,
            entry_index,
        });
    }

    if conflict_count > 1 {
        return Ok((None, conflict_count));
    }
    if atoms.is_empty() {
        return Ok((None, 0));
    }

    atoms.sort_by_key(|atom| atom.key);
    Ok((Some(LogicalProfile { atoms }), 0))
}

fn logical_profiles_exactly_equal(
    package: &DBPF<'_>,
    left: &LogicalProfile,
    right: &LogicalProfile,
) -> bool {
    if left.atoms.len() != right.atoms.len() {
        return false;
    }

    let left_keys: Vec<LogicalAtomKey> = left.atoms.iter().map(|atom| atom.key).collect();
    let right_keys: Vec<LogicalAtomKey> = right.atoms.iter().map(|atom| atom.key).collect();
    if left_keys != right_keys {
        return false;
    }

    let mut start = 0usize;
    while start < left.atoms.len() {
        let atom_key = left.atoms[start].key;
        let mut end = start + 1;
        while end < left.atoms.len() && left.atoms[end].key == atom_key {
            end += 1;
        }

        let mut used = vec![false; end - start];
        for left_atom in &left.atoms[start..end] {
            let left_data = package.files[left_atom.entry_index].data();
            let mut matched = false;

            for (offset, right_atom) in right.atoms[start..end].iter().enumerate() {
                if used[offset] {
                    continue;
                }
                let right_data = package.files[right_atom.entry_index].data();
                if left_data == right_data {
                    used[offset] = true;
                    matched = true;
                    break;
                }
            }

            if !matched {
                return false;
            }
        }

        start = end;
    }

    true
}

fn build_resource_relations(
    package: &DBPF<'_>,
    name_map: &BTreeMap<u64, String>,
    items: &[ResourceItem],
) -> BTreeMap<String, ResourceRelation> {
    let tgi_index = relation_tgi_index(package);
    let tgi_conflicts = relation_tgi_conflicts(package, &tgi_index);
    let mut atom_cache: HashMap<usize, LogicalAtomKey> = HashMap::new();
    let mut profiles: Vec<Option<LogicalProfile>> = vec![None; items.len()];
    let mut relations: Vec<ResourceRelation> = vec![ResourceRelation::default(); items.len()];

    for (item_index, item) in items.iter().enumerate() {
        let Ok((profile, conflict_count)) = logical_profile_for_item(
            package,
            item,
            name_map,
            &tgi_index,
            &tgi_conflicts,
            &mut atom_cache,
        )
        else {
            continue;
        };
        if conflict_count > 1 {
            relations[item_index].tgi_conflict_count = conflict_count;
        } else {
            profiles[item_index] = profile;
        }
    }

    let mut buckets: HashMap<Vec<LogicalAtomKey>, Vec<usize>> = HashMap::new();
    for (item_index, profile) in profiles.iter().enumerate() {
        let Some(profile) = profile else {
            continue;
        };
        let summary: Vec<LogicalAtomKey> = profile.atoms.iter().map(|atom| atom.key).collect();
        buckets.entry(summary).or_default().push(item_index);
    }

    for candidates in buckets.values().filter(|candidates| candidates.len() > 1) {
        let mut remaining = candidates.clone();

        while let Some(seed) = remaining.pop() {
            let Some(seed_profile) = profiles[seed].as_ref() else {
                continue;
            };

            let mut exact_group = vec![seed];
            let mut keep = Vec::new();

            for candidate in remaining {
                let Some(candidate_profile) = profiles[candidate].as_ref() else {
                    keep.push(candidate);
                    continue;
                };

                if logical_profiles_exactly_equal(package, seed_profile, candidate_profile) {
                    exact_group.push(candidate);
                } else {
                    keep.push(candidate);
                }
            }

            remaining = keep;

            if exact_group.len() > 1 {
                let duplicate_count = exact_group.len();
                for item_index in exact_group {
                    relations[item_index].duplicate_count = duplicate_count;
                }
            }
        }
    }

    let mut by_key = BTreeMap::new();
    for (item, relation) in items.iter().zip(relations.into_iter()) {
        if relation.duplicate_count > 1 || relation.tgi_conflict_count > 1 {
            by_key.insert(item.key.clone(), relation);
        }
    }

    by_key
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

    // A consolidated PoseList XML/S3SA pair is a derived output and is
    // intentionally not owned by any original source package in the manifest.
    // If the user explicitly selects such a resource, remove it unless a
    // remaining source package still owns/requires that same TGI.
    for key in &selected_atomic {
        if available.contains(key) && !remaining_resources.contains(key) {
            selected_resources.insert(key.clone());
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

    if let Some(manifest) = read_valid_merge_manifest_from_dbpf(package) {
        if let Some(plan) = manifest_removal_plan_sets(package, &manifest, selected_keys) {
            return Ok(plan);
        }
    }

    let selected_compound: HashSet<String> = selected_keys.iter().cloned().collect();
    let selected_resources = expand_compound_keys(&selected_compound);
    let mut candidate = expand_selection_for_export(package, &selected_compound, true, name_map)?;

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

    let required_by_remaining = collect_clone_keys(package, &remaining_seeds, true, name_map)?;
    let shared_preserved: HashSet<String> = candidate
        .intersection(&required_by_remaining)
        .cloned()
        .collect();

    let resources_to_remove: HashSet<String> =
        candidate.difference(&shared_preserved).cloned().collect();

    Ok((selected_resources, shared_preserved, resources_to_remove))
}

// A valid Veiga Merge Manifest already identifies which source owns every
// selected package and resource. Decoding every CASP/OBJD name in a 1 GiB
// merged is unnecessary for this path; retain the full map for selections
// that need generic dependency analysis (including derived resources).
fn removal_name_map(
    package: &DBPF<'_>,
    selected_keys: &[String],
) -> BTreeMap<u64, String> {
    if let Some(manifest) = read_valid_merge_manifest_from_dbpf(package) {
        if manifest_removal_plan_sets(package, &manifest, selected_keys).is_some() {
            return BTreeMap::new();
        }
    }
    package_resource_names(package)
}

fn build_remove_selection_plan(
    package: &DBPF<'_>,
    selected_keys: &[String],
    name_map: &BTreeMap<u64, String>,
) -> Result<RemoveSelectionPlan, String> {
    let merge_manifest = read_valid_merge_manifest_from_dbpf(package);
    let manifest_backed = merge_manifest.is_some();
    let manifest_package_total = merge_manifest
        .as_ref()
        .map(|manifest| manifest.packages.len())
        .unwrap_or(0);
    let manifest_packages_to_remove = merge_manifest
        .as_ref()
        .map(|manifest| manifest.selected_package_ids(selected_keys).len())
        .unwrap_or(0);
    let (selected_resources, shared_preserved, resources_to_remove) =
        removal_plan_sets(package, selected_keys, name_map)?;

    // The generic removal closure is partitioned into removable and shared
    // preserved resources. Their disjoint union is the exact candidate set;
    // recalculating the entire dependency graph here doubled planning time.
    let candidate_resources = if manifest_backed {
        selected_resources.len()
    } else {
        resources_to_remove.len() + shared_preserved.len()
    };
    let selected_resources_preserved = selected_resources.intersection(&shared_preserved).count();

    Ok(RemoveSelectionPlan {
        selected_items: selected_keys.len(),
        selected_resources: selected_resources.len(),
        candidate_resources,
        shared_resources_preserved: shared_preserved.len(),
        selected_resources_preserved,
        resources_to_remove: resources_to_remove.len(),
        manifest_packages_to_remove,
        manifest_package_total,
        remaining_resources: package
            .files
            .iter()
            .filter(|entry| entry.resource_type != TYPE_KEY)
            .filter(|entry| {
                !is_manifest_tgi(entry.resource_type, entry.resource_group, entry.instance)
            })
            .filter(|entry| !resources_to_remove.contains(&key_string(entry)))
            .count(),
    })
}

fn available_resource_keys(package: &DBPF<'_>) -> HashSet<String> {
    package
        .files
        .iter()
        .filter(|entry| !is_manifest_tgi(entry.resource_type, entry.resource_group, entry.instance))
        .map(key_string)
        .collect()
}

fn read_valid_merge_manifest_from_dbpf(package: &DBPF<'_>) -> Option<MergeManifest> {
    let manifest = read_merge_manifest_from_dbpf(package).ok().flatten()?;
    manifest
        .validate_keys(&available_resource_keys(package))
        .ok()?;
    Some(manifest)
}

fn require_valid_veiga_merge_manifest(package: &DBPF<'_>) -> Result<(), String> {
    if read_valid_merge_manifest_from_dbpf(package).is_some() {
        Ok(())
    } else {
        Err("Remove from Merged requer um Veiga Merge Manifest válido.".to_string())
    }
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

fn copy_backup_with_progress(
    app: Option<&AppHandle>,
    started: Instant,
    source_path: &Path,
    backup_path: &Path,
) -> Result<(), String> {
    let total_bytes = std::fs::metadata(source_path)
        .map_err(|e| e.to_string())?
        .len();
    let mut original = File::open(source_path).map_err(|e| e.to_string())?;
    let mut backup = File::create(backup_path).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    let mut copied = 0u64;
    let mut last_progress = Instant::now();

    loop {
        let count = original.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        backup.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
        copied += count as u64;
        if last_progress.elapsed() >= Duration::from_millis(400) {
            let percent = 75 + (copied.saturating_mul(12) / total_bytes.max(1)) as u32;
            let message = format!(
                "Criando backup: {} de {} MiB.",
                copied / (1024 * 1024),
                total_bytes / (1024 * 1024),
            );
            emit_package_progress(
                app, started, "Criando backup", &message,
                percent.min(87), 100, 0, 0,
            );
            last_progress = Instant::now();
        }
    }
    backup.flush().map_err(|e| e.to_string())?;
    if copied != total_bytes {
        return Err("O tamanho do original mudou durante o backup.".to_string());
    }
    Ok(())
}

fn files_are_identical(
    app: Option<&AppHandle>,
    started: Instant,
    left: &Path,
    right: &Path,
) -> Result<bool, String> {
    let total_bytes = std::fs::metadata(left).map_err(|e| e.to_string())?.len();
    let mut a = File::open(left).map_err(|e| e.to_string())?;
    let mut b = File::open(right).map_err(|e| e.to_string())?;
    let mut a_buf = vec![0u8; 4 * 1024 * 1024];
    let mut b_buf = vec![0u8; 4 * 1024 * 1024];
    let mut compared = 0u64;
    let mut last_progress = Instant::now();

    loop {
        let a_len = a.read(&mut a_buf).map_err(|e| e.to_string())?;
        let b_len = b.read(&mut b_buf).map_err(|e| e.to_string())?;
        if a_len != b_len {
            return Ok(false);
        }
        if a_len == 0 {
            return Ok(compared == total_bytes);
        }
        if a_buf[..a_len] != b_buf[..b_len] {
            return Ok(false);
        }
        compared += a_len as u64;
        if last_progress.elapsed() >= Duration::from_millis(400) {
            let percent = 88 + (compared.saturating_mul(7) / total_bytes.max(1)) as u32;
            let message = format!(
                "Verificando backup: {} de {} MiB.",
                compared / (1024 * 1024),
                total_bytes / (1024 * 1024),
            );
            emit_package_progress(
                app, started, "Verificando backup", &message,
                percent.min(95), 100, 0, 0,
            );
            last_progress = Instant::now();
        }
    }
}


fn package_tgi_payload_signature(
    path: &Path,
) -> Result<Vec<(u32, u32, u64, Vec<u8>)>, String> {
    let package = WritablePackage::load(path)
        .map_err(|e| format!("Não foi possível analisar {} para detectar duplicados: {e}", path.display()))?;
    let mut signature = Vec::new();

    for entry in &package.entries {
        let parts = ResourceKeyParts {
            resource_type: entry.type_id,
            resource_group: entry.group,
            instance: entry.instance,
        };
        if relation_ignored_resource(parts) {
            continue;
        }

        let data = package.get_uncompressed_data(entry).map_err(|e| {
            format!(
                "Não foi possível ler o recurso {} de {} durante a detecção de duplicados: {e}",
                key_string_from_writable(entry),
                path.display()
            )
        })?;
        signature.push((entry.type_id, entry.group, entry.instance, data));
    }

    signature.sort();
    Ok(signature)
}

fn packages_have_same_tgi_payloads(left: &Path, right: &Path) -> Result<bool, String> {
    let left_signature = package_tgi_payload_signature(left)?;
    if left_signature.is_empty() {
        return Ok(false);
    }

    let right_signature = package_tgi_payload_signature(right)?;
    if right_signature.is_empty() {
        return Ok(false);
    }

    Ok(left_signature == right_signature)
}

fn existing_package_paths(destination: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = std::fs::read_dir(destination)
        .map_err(|e| format!("Não foi possível ler a pasta de destino {}: {e}", destination.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("package"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    paths.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    Ok(paths)
}

fn unique_restored_output_path(preferred: &Path) -> PathBuf {
    if !preferred.exists() {
        return preferred.to_path_buf();
    }

    let parent = preferred.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = preferred
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "Package".to_string());
    let extension = preferred
        .extension()
        .map(|ext| ext.to_string_lossy().to_string())
        .unwrap_or_else(|| "package".to_string());

    for suffix in 2usize.. {
        let candidate = parent.join(format!("{stem} ({suffix}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}

fn unique_restore_temp_path(destination: &Path, package_id: u32) -> PathBuf {
    let base = destination.join(format!(
        ".veiga-unmerge-{}-{package_id}.package",
        std::process::id()
    ));
    if !base.exists() {
        return base;
    }

    for suffix in 2usize.. {
        let candidate = destination.join(format!(
            ".veiga-unmerge-{}-{package_id}-{suffix}.package",
            std::process::id()
        ));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}

fn merger_anchor_kind(resource_type: u32) -> Option<&'static str> {
    match resource_type {
        TYPE_CASP => Some("CASP"),
        TYPE_OBJD => Some("OBJD"),
        TYPE_FACE => Some("FACE"),
        TYPE_SKINTONE => Some("SkinTone"),
        TYPE_PATTERN => Some("Pattern"),
        TYPE_CLIP => Some("CLIP"),
        _ => None,
    }
}

fn normalized_merged_output_name(name: &str) -> String {
    let trimmed = name.trim();
    let base = if trimmed.is_empty() {
        "Merged.package"
    } else {
        trimmed
    };
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

fn is_per_source_metadata_type(resource_type: u32) -> bool {
    matches!(resource_type, TYPE_KEY | TYPE_PACKAGE_MANIFEST)
}

fn preserved_source_metadata(
    source: &WritablePackage,
) -> Result<Vec<ManifestPreservedResource>, String> {
    let mut preserved = Vec::new();
    for entry in &source.entries {
        if !is_per_source_metadata_type(entry.type_id) {
            continue;
        }
        let data = source.get_uncompressed_data(entry).map_err(|e| {
            format!(
                "Não foi possível preservar metadado {}: {e}",
                key_string_from_writable(entry)
            )
        })?;
        preserved.push(ManifestPreservedResource {
            key: key_string_from_writable(entry),
            data_base64: STANDARD.encode(data),
            unknown2: entry.unknown2,
        });
    }
    Ok(preserved)
}

fn source_manifest_entries(
    source: &WritablePackage,
    source_path: &Path,
    next_id: &mut u32,
) -> Result<Vec<ManifestPackage>, String> {
    let existing = read_merge_manifest_from_writable(source)?;
    let mut available: HashSet<String> = source
        .entries
        .iter()
        .filter(|entry| !is_per_source_metadata_type(entry.type_id))
        .filter(|entry| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
        .map(key_string_from_writable)
        .collect();

    // A merged PoseList output reuses the primary XML/S3SA TGI, while the
    // manifest stores the original resources as preserved data. Do not claim
    // that derived output entry as a new source resource when merging again.
    if let Some(manifest) = &existing {
        let preserved_keys: HashSet<String> = manifest
            .packages
            .iter()
            .flat_map(|package| package.preserved_resources.iter())
            .map(|resource| resource.key.clone())
            .collect();
        available.retain(|key| !preserved_keys.contains(key));
    }

    if let Some(existing) = existing {
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
            if resources.is_empty() && package.preserved_resources.is_empty() {
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
                preserved_resources: package.preserved_resources,
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
                preserved_resources: Vec::new(),
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
            if entry.type_id == TYPE_KEY
                || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
            {
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
        preserved_resources: preserved_source_metadata(source)?,
    };
    *next_id += 1;
    Ok(vec![package])
}

fn is_merge_source_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "package" | "dbc"))
            .unwrap_or(false)
}

fn collect_merge_sources_from_path(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("Não foi possível acessar {}: {e}", path.display()))?;

    if metadata.file_type().is_symlink() {
        return Ok(());
    }

    if metadata.is_file() {
        if is_merge_source_file(path) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    if !metadata.is_dir() {
        return Ok(());
    }

    let mut entries = std::fs::read_dir(path)
        .map_err(|e| format!("Não foi possível ler a pasta {}: {e}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Falha ao enumerar a pasta {}: {e}", path.display()))?;

    entries.sort_by_key(|entry| entry.path().to_string_lossy().to_ascii_lowercase());
    for entry in entries {
        collect_merge_sources_from_path(&entry.path(), files)?;
    }

    Ok(())
}

#[tauri::command]
pub fn collect_merge_package_files(
    payload: CollectMergeSourcesPayload,
) -> Result<Vec<String>, String> {
    let mut files = Vec::new();

    for raw_path in payload.paths {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            continue;
        }
        collect_merge_sources_from_path(Path::new(trimmed), &mut files)?;
    }

    files.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());

    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for path in files {
        let display = path.to_string_lossy().into_owned();
        let normalized = display.replace('\\', "/").to_ascii_lowercase();
        if seen.insert(normalized) {
            result.push(display);
        }
    }

    Ok(result)
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
    let output_name = normalized_merged_output_name(&payload.output_name);
    let output_path = destination.join(&output_name);
    let overwrite_target = output_path.exists();
    if overwrite_target && !payload.allow_overwrite {
        return Err(format!(
            "Já existe um arquivo com esse nome no destino: {}. Confirme a substituição na janela Salvar como.",
            output_path.display()
        ));
    }
    if overwrite_target {
        // Never replace an input package during a merge, even after a Save dialog
        // confirmed overwrite: an original input can still be open/read below.
        let existing_output = output_path
            .canonicalize()
            .map_err(|e| format!("Não foi possível validar o arquivo de destino: {e}"))?;
        for source_path in &payload.package_paths {
            let input_path = PathBuf::from(source_path);
            if input_path.canonicalize().ok().as_ref() == Some(&existing_output) {
                return Err(format!(
                    "O destino selecionado também é um package de origem: {}. Escolha outro nome para preservar o original.",
                    output_path.display()
                ));
            }
        }
    }
    // Never unlink an existing path just because it resembles our former
    // process-ID temp filename: it might belong to the user or another job.
    let stamp = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp_path = (0..128)
        .map(|counter| {
            destination.join(format!(
                ".{}.merge-temp-{}-{stamp}-{counter}.package",
                output_name.trim_end_matches(".package"),
                std::process::id()
            ))
        })
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| "Não foi possível reservar um nome temporário seguro para a mesclagem.".to_string())?;

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

    let pose_list_plan = pose_list_merge_plan(&sources)?;

    let mut first_owner: HashMap<String, (usize, usize)> = HashMap::new();
    let mut shared_keys = HashSet::new();
    for (source_index, source) in sources.iter().enumerate() {
        for (entry_index, entry) in source.entries.iter().enumerate() {
            if is_per_source_metadata_type(entry.type_id)
                || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
            {
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
                    let previous_is_pose_resource = pose_list_plan
                        .as_ref()
                        .map(|plan| plan.entries_to_skip.contains(&(previous_source, previous_entry)))
                        .unwrap_or(false);
                    let current_is_pose_resource = pose_list_plan
                        .as_ref()
                        .map(|plan| plan.entries_to_skip.contains(&(source_index, entry_index)))
                        .unwrap_or(false);
                    if previous_is_pose_resource && current_is_pose_resource {
                        // PoseList XMLs and their S3SA registrars are deliberately
                        // consolidated into one output pair; retain originals in the manifest.
                        shared_keys.insert(key);
                        continue;
                    }

                    // Same Type/Group/Instance with different *decoded* data
                    // is not an identical resource. Never pick a winner silently:
                    // recolors and overrides can deliberately share TGI values.
                    let left = source_paths[previous_source].display();
                    let right = source_paths[source_index].display();
                    return Err(format!(
                        "Conflito de TGI: {key} ({kind}, Type 0x{type_id:08X}, Group 0x{group:08X}, Instance 0x{instance:016X}).\n\
                         Origem A: {left}\n  Tamanho descompactado: {size_a} bytes\n\
                         Origem B: {right}\n  Tamanho descompactado: {size_b} bytes\n\
                         Os dois recursos têm a mesma identidade, mas conteúdos diferentes. O merge foi cancelado sem substituir nenhum original. Separe os CCs, ou decida qual override/recolor deve prevalecer antes de criar um merged.",
                        kind = kind_from_type(entry.type_id),
                        type_id = entry.type_id,
                        group = entry.group,
                        instance = entry.instance,
                        size_a = previous_data.len(),
                        size_b = current_data.len(),
                    ));
                }
                shared_keys.insert(key);
            } else {
                first_owner.insert(key, (source_index, entry_index));
            }
        }
    }

    let mut manifest_packages = Vec::new();
    let mut source_manifest_ranges = Vec::new();
    let mut next_id = 1u32;
    for (source, source_path) in sources.iter().zip(source_paths.iter()) {
        let start = manifest_packages.len();
        manifest_packages.extend(source_manifest_entries(source, source_path, &mut next_id)?);
        source_manifest_ranges.push((start, manifest_packages.len()));
    }

    if let Some(plan) = &pose_list_plan {
        for (source_index, preserved) in &plan.preserved_resources {
            let (start, end) = source_manifest_ranges[*source_index];
            let already_preserved = manifest_packages[start..end].iter().any(|package| {
                package
                    .preserved_resources
                    .iter()
                    .any(|resource| resource.key == preserved.key)
            });

            if already_preserved {
                // This is the derived PoseList output of an earlier reversible merge.
                // Keep the earlier original data, not the already-combined XML/S3SA bytes.
                for package in &mut manifest_packages[start..end] {
                    package.resources.retain(|resource| resource.key != preserved.key);
                    package.anchors.retain(|anchor| anchor.key != preserved.key);
                }
                continue;
            }

            let owners: Vec<usize> = (start..end)
                .filter(|index| {
                    manifest_packages[*index]
                        .resources
                        .iter()
                        .any(|resource| resource.key == preserved.key)
                })
                .collect();
            if owners.is_empty() {
                return Err(format!(
                    "Não foi possível associar o recurso PoseList original {} ao Merge Manifest. O merge foi cancelado para preservar o Unmerge.",
                    preserved.key
                ));
            }
            for index in owners {
                let package = &mut manifest_packages[index];
                package.resources.retain(|resource| resource.key != preserved.key);
                package.anchors.retain(|anchor| anchor.key != preserved.key);
                package.preserved_resources.push(preserved.clone());
            }
        }
    }

    let manifest = MergeManifest {
        version: 2,
        packages: manifest_packages,
    };

    let mut out_pkg = WritablePackage::new(&temp_path, false).map_err(|e| e.to_string())?;
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
            if is_per_source_metadata_type(entry.type_id)
                || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
            {
                continue;
            }
            let key = key_string_from_writable(entry);
            if pose_list_plan
                .as_ref()
                .map(|plan| {
                    plan.entries_to_skip.contains(&(source_index, entry_index))
                        || plan.output_keys.contains(&key)
                })
                .unwrap_or(false)
            {
                continue;
            }
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

    if let Some(plan) = &pose_list_plan {
        let (xml_source_index, xml_entry_index) = plan.primary_xml;
        let xml_entry = &sources[xml_source_index].entries[xml_entry_index];
        append_uncompressed_resource(
            &mut out_pkg,
            &mut out_f,
            TYPE_XML,
            xml_entry.group,
            xml_entry.instance,
            &plan.combined_xml,
            xml_entry.unknown2,
        )?;
        written.insert(key_string_from_writable(xml_entry));

        let (registrar_source_index, registrar_entry_index) = plan.primary_registrar;
        let registrar_source = &sources[registrar_source_index];
        let registrar = &registrar_source.entries[registrar_entry_index];
        let raw = registrar_source
            .get_raw_data(registrar)
            .map_err(|e| format!("Não foi possível copiar o registrar S3SA principal: {e}"))?;
        let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_f.write_all(&raw).map_err(|e| e.to_string())?;
        out_pkg.entries.push(WritableEntry {
            type_id: registrar.type_id,
            group: registrar.group,
            id1: registrar.id1,
            id2: registrar.id2,
            instance: registrar.instance,
            chunk_offset,
            file_size: registrar.file_size,
            mem_size: registrar.mem_size,
            compressed: registrar.compressed,
            unknown2: registrar.unknown2,
        });
        written.insert(key_string_from_writable(registrar));
    }

    if let Some(key_data) = merged_clip_key_data(&sources)? {
        append_uncompressed_resource(
            &mut out_pkg,
            &mut out_f,
            TYPE_KEY,
            0,
            0,
            &key_data,
            0,
        )?;
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

    // Sorting the index does not reorder any compressed payload or chunk offset.
    // Keep DBPF lookup deterministic and avoid arbitrary source-order entries.
    out_pkg.entries.sort_by_key(|entry| (entry.type_id, entry.group, entry.instance));
    out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
    out_pkg
        .write_index_and_header()
        .map_err(|e| e.to_string())?;
    drop(out_f);
    drop(out_pkg);

    let rebuilt = WritablePackage::load(&temp_path)
        .map_err(|e| format!("O merged temporário falhou na validação DBPF: {e}"))?;
    let rebuilt_manifest = read_merge_manifest_from_writable(&rebuilt)?
        .ok_or_else(|| "O merged final não contém o Merge Manifest.".to_string())?;
    let available_keys: HashSet<String> = rebuilt
        .entries
        .iter()
        .filter(|entry| !is_manifest_tgi(entry.type_id, entry.group, entry.instance))
        .map(key_string_from_writable)
        .collect();
    rebuilt_manifest.validate_keys(&available_keys)?;
    drop(rebuilt);

    let backup_path = if overwrite_target {
        // The native Save dialog may have approved overwrite. Only replace after
        // validating the complete temporary DBPF and its merge manifest. Keep a
        // permanent rollback backup with a non-.package suffix so recursive
        // source-folder scans cannot silently re-import the backup as CC.
        if !output_path.exists() {
            return Err("O package de destino desapareceu durante a mesclagem; nenhuma substituição foi realizada.".to_string());
        }
        let backup_dir = destination.join("Merged_backups");
        create_dir_all(&backup_dir)
            .map_err(|e| format!("Não foi possível criar a pasta de backups do Merger: {e}"))?;
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let original_name = output_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Merged.package");
        let backup_path = (0..128)
            .map(|counter| backup_dir.join(format!(
                "{original_name}.{stamp}.{}.{}.s3ccbak",
                std::process::id(),
                counter
            )))
            .find(|candidate| !candidate.exists())
            .ok_or_else(|| "Não foi possível obter um nome de backup exclusivo; o original foi preservado.".to_string())?;
        replace_source_with_rollback(&output_path, &temp_path, &backup_path)?;
        Some(backup_path.display().to_string())
    } else {
        // Refuse to overwrite a file that appeared while the merge was running.
        if output_path.exists() {
            return Err("Um package surgiu no caminho de destino durante a mesclagem; nenhum arquivo foi substituído.".to_string());
        }
        rename(&temp_path, &output_path).map_err(|e| {
            let _ = remove_file(&temp_path);
            format!("O merged foi validado, mas não foi possível finalizar o arquivo: {e}")
        })?;
        None
    };

    Ok(MergePackagesResult {
        output_path: output_path.display().to_string(),
        backup_path,
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

    let mut planned_names = HashSet::new();
    for package_info in &manifest.packages {
        let mut file_name = sanitize_file_name(&package_info.name);
        if file_name.trim().is_empty() {
            file_name = format!("Package_{}.package", package_info.id);
        }
        if !file_name.to_ascii_lowercase().ends_with(".package") {
            file_name.push_str(".package");
        }
        let normalized_name = file_name.to_ascii_lowercase();
        if !planned_names.insert(normalized_name) {
            return Err(format!(
                "O manifesto contém nomes de arquivo duplicados que não podem ser restaurados na mesma pasta: {file_name}"
            ));
        }

        for resource in &package_info.resources {
            if !index.contains_key(&resource.key) {
                return Err(format!(
                    "A restauração foi cancelada antes de criar arquivos: {} referencia o recurso ausente {}.",
                    package_info.name, resource.key
                ));
            }
        }
        for resource in &package_info.preserved_resources {
            parse_key(&resource.key).map_err(|e| {
                format!(
                    "A restauração foi cancelada: metadado preservado inválido em {} ({}): {e}",
                    package_info.name, resource.key
                )
            })?;
            STANDARD.decode(&resource.data_base64).map_err(|e| {
                format!(
                    "A restauração foi cancelada: metadado preservado corrompido em {} ({}): {e}",
                    package_info.name, resource.key
                )
            })?;
        }
    }

    let mut existing_packages = existing_package_paths(&destination)?;
    let mut output_paths = Vec::new();
    let mut duplicate_paths = Vec::new();

    for package_info in &manifest.packages {
        let mut file_name = sanitize_file_name(&package_info.name);
        if file_name.trim().is_empty() {
            file_name = format!("Package_{}.package", package_info.id);
        }
        if !file_name.to_ascii_lowercase().ends_with(".package") {
            file_name.push_str(".package");
        }

        let temp_path = unique_restore_temp_path(&destination, package_info.id);
        let mut out_pkg = WritablePackage::new(&temp_path, false).map_err(|e| e.to_string())?;
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

        for resource in &package_info.preserved_resources {
            let parts = parse_key(&resource.key).map_err(|e| {
                format!(
                    "Não foi possível restaurar {}: metadado preservado {} é inválido: {e}",
                    package_info.name, resource.key
                )
            })?;
            let data = STANDARD.decode(&resource.data_base64).map_err(|e| {
                format!(
                    "Não foi possível restaurar {}: metadado preservado {} está corrompido: {e}",
                    package_info.name, resource.key
                )
            })?;
            append_uncompressed_resource(
                &mut out_pkg,
                &mut out_f,
                parts.resource_type,
                parts.resource_group,
                parts.instance,
                &data,
                resource.unknown2,
            )?;
        }

        out_pkg.index_position = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        out_pkg
            .write_index_and_header()
            .map_err(|e| e.to_string())?;
        drop(out_f);
        drop(out_pkg);

        if let Err(error) = WritablePackage::load(&temp_path) {
            let _ = remove_file(&temp_path);
            return Err(format!("Package restaurado inválido ({}): {error}", file_name));
        }

        let mut duplicate_path = None;
        for existing_path in &existing_packages {
            match packages_have_same_tgi_payloads(&temp_path, existing_path) {
                Ok(true) => {
                    duplicate_path = Some(existing_path.clone());
                    break;
                }
                Ok(false) => {}
                Err(_) => {
                    // An unrelated invalid/inaccessible package must not block Unmerge.
                    // It simply cannot participate in duplicate detection.
                }
            }
        }

        if let Some(existing_path) = duplicate_path {
            remove_file(&temp_path).map_err(|e| {
                format!(
                    "Foi detectado um package duplicado, mas não foi possível remover o temporário {}: {e}",
                    temp_path.display()
                )
            })?;
            duplicate_paths.push(existing_path.display().to_string());
            continue;
        }

        let preferred_output_path = destination.join(&file_name);
        let output_path = unique_restored_output_path(&preferred_output_path);
        rename(&temp_path, &output_path).map_err(|e| {
            let _ = remove_file(&temp_path);
            format!(
                "O package restaurado foi validado, mas não foi possível finalizar {}: {e}",
                output_path.display()
            )
        })?;

        existing_packages.push(output_path.clone());
        output_paths.push(output_path.display().to_string());
    }

    Ok(RestoreManifestResult {
        destination_folder: destination.display().to_string(),
        package_count: output_paths.len(),
        duplicate_count: duplicate_paths.len(),
        output_paths,
        duplicate_paths,
    })
}

fn write_removed_package(
    app: Option<&AppHandle>,
    started: Instant,
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

    // Keep memory bounded even if a single resource occupies hundreds of MiB.
    // Only resources retained in the rebuilt package need to be copied.
    let total_bytes = source_package.entries.iter()
        .filter(|entry| {
            entry.type_id != TYPE_KEY
                && !is_manifest_tgi(entry.type_id, entry.group, entry.instance)
                && !resources_to_remove.contains(&key_string_from_writable(entry))
        })
        .fold(0u64, |total, entry| total.saturating_add(entry.file_size as u64));
    let mut copied_bytes = 0u64;
    let mut last_progress = Instant::now();
    let mut copy_buffer = vec![0u8; 4 * 1024 * 1024];

    for entry in &source_package.entries {
        let key = key_string_from_writable(entry);
        if entry.type_id == TYPE_KEY
            || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
            || resources_to_remove.contains(&key)
        {
            continue;
        }

        let chunk_offset = out_f.stream_position().map_err(|e| e.to_string())? as u32;
        source_file
            .seek(SeekFrom::Start(entry.chunk_offset as u64))
            .map_err(|e| e.to_string())?;
        let mut remaining = entry.file_size as u64;
        while remaining > 0 {
            let chunk_size = copy_buffer.len().min(remaining as usize);
            source_file.read_exact(&mut copy_buffer[..chunk_size]).map_err(|e| e.to_string())?;
            out_f.write_all(&copy_buffer[..chunk_size]).map_err(|e| e.to_string())?;
            remaining -= chunk_size as u64;
            copied_bytes += chunk_size as u64;

            if last_progress.elapsed() >= Duration::from_millis(400) {
                let percent = 25 + (copied_bytes.saturating_mul(40) / total_bytes.max(1)) as u32;
                let message = format!(
                    "Gravando recursos preservados: {} de {} MiB.",
                    copied_bytes / (1024 * 1024),
                    total_bytes / (1024 * 1024)
                );
                emit_package_progress(
                    app, started, "Reconstruindo merged", &message,
                    percent.min(65), 100, out_pkg.entries.len(), 0,
                );
                last_progress = Instant::now();
            }
        }
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

    // Sorting the index does not reorder any compressed payload or chunk offset.
    // Keep DBPF lookup deterministic and avoid arbitrary source-order entries.
    out_pkg.entries.sort_by_key(|entry| (entry.type_id, entry.group, entry.instance));
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
        if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
        {
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
        if entry.type_id == TYPE_KEY || is_manifest_tgi(entry.type_id, entry.group, entry.instance)
        {
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
                return Err(
                    "Validação falhou: o Merge Manifest deveria ter sido removido.".to_string(),
                );
            }
        }
        Some(expected) => {
            let actual = actual_manifest.ok_or_else(|| {
                "Validação falhou: o Merge Manifest atualizado está ausente.".to_string()
            })?;
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
    backup_path: &Path,
) -> Result<(), String> {
    // Same-volume rename preserves the original bytes without copying 1+ GiB
    // or comparing that data again. The source is only moved after the rebuilt
    // package passed structural and manifest validation.
    if backup_path.exists() {
        return Err(format!(
            "O caminho do backup já existe: {}",
            backup_path.display()
        ));
    }

    rename(source_path, backup_path).map_err(|e| {
        format!(
            "Não foi possível mover o merged original para o backup; nenhum arquivo foi substituído: {e}"
        )
    })?;
    if let Err(error) = rename(temp_path, source_path) {
        if let Err(rollback_error) = rename(backup_path, source_path) {
            return Err(format!(
                "Falha ao substituir o merged ({error}) e ao restaurá-lo ({rollback_error}). O original permanece no backup em {}.",
                backup_path.display()
            ));
        }
        return Err(format!(
            "Não foi possível instalar o merged reconstruído; o original foi restaurado sem alterações: {error}"
        ));
    }
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
        for parts in &parsed {
            if parts.resource_type != TYPE_XML {
                continue;
            }
            if let Some(entry) = package.files.iter().find(|entry| {
                entry.resource_type == parts.resource_type
                    && entry.resource_group == parts.resource_group
                    && entry.instance == parts.instance
            }) {
                if is_pose_list_xml(entry) {
                    let text = String::from_utf8_lossy(entry.data());
                    return sanitize_file_name(&pose_list_display_label(&text, entry.instance));
                }
            }
        }

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
    format!(
        "{}.package",
        selection_output_stem(package, base_key, name_map)
    )
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
    source_file
        .read_exact(&mut raw)
        .map_err(|e| e.to_string())?;
    Ok(raw)
}

fn writable_entry_index(source_package: &WritablePackage) -> HashMap<String, usize> {
    let mut index = HashMap::with_capacity(source_package.entries.len());
    for (position, entry) in source_package.entries.iter().enumerate() {
        index
            .entry(key_string_from_writable(entry))
            .or_insert(position);
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
        let kind = if categories.iter().any(|category| category == "Pose")
            && !kinds.is_empty()
            && kinds.iter().all(|kind| kind == "CLIP")
        {
            "Pose".to_string()
        } else if kinds.is_empty() {
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
pub async fn scan_package_resources(app: AppHandle, payload: ScanPayload) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan_package_resources_inner(Some(&app), payload))
        .await
        .map_err(|error| format!("Falha ao executar a leitura do package: {error}"))?
}

fn scan_package_resources_inner(
    app: Option<&AppHandle>,
    payload: ScanPayload,
) -> Result<ScanResult, String> {
    let package_path = payload.package_path.trim();
    if package_path.is_empty() {
        return Err("Nenhum package foi selecionado.".to_string());
    }
    validate_package_extension(package_path)?;

    let mode_raw = payload.resource_type.trim().to_lowercase();
    let resource_type = match mode_raw.as_str() {
        "" => "all".to_string(),
        "all" | "casp" | "face" | "skintone" | "pattern" | "object" | "clip" => mode_raw,
        _ => "all".to_string(),
    };

    CANCEL_REQUESTED.store(false, Ordering::Release);
    let started = Instant::now();
    emit_package_progress(
        app, started, "Opening package",
        "Mapping package data without copying all resource payloads.", 0, 5, 0, 0,
    );

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mmap = open_package(package_path)?;
        check_cancelled()?;
        let package = parse_package(&mmap)?;
        check_cancelled()?;
        emit_package_progress(
            app, started, "Reading names",
            "Reading package metadata and resource names.", 1, 5, package.files.len(), mmap.len(),
        );
        let query = payload.search.unwrap_or_default().to_lowercase();
        let name_map = package_resource_names(&package);
        check_cancelled()?;
        let merge_manifest = read_valid_merge_manifest_from_dbpf(&package);
        let manifest_package_count = merge_manifest
            .as_ref()
            .map(|manifest| manifest.packages.len())
            .unwrap_or(0);

        emit_package_progress(
            app, started, "Building resource list",
            "Building and sorting resource metadata.", 2, 5, package.files.len(), mmap.len(),
        );
        check_cancelled()?;
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
        } else if resource_type == "clip" {
            build_clip_group_items(&package, &query, &name_map)
        } else if resource_type == "all" {
            let mut items: Vec<ResourceItem> = package
                .files
                .iter()
                .filter(|entry| filter_matches(entry, "all"))
                .filter(|entry| entry.resource_type != TYPE_SKINTONE)
                .filter(|entry| entry.resource_type != TYPE_FACE)
                .filter(|entry| entry.resource_type != TYPE_XML)
                .filter(|entry| entry.resource_type != TYPE_CASP)
                .filter(|entry| entry.resource_type != TYPE_CLIP)
                .map(|entry| build_item(&package, entry, &name_map))
                .filter(|item| item_matches_query(item, &query))
                .collect();

            items.extend(build_casp_group_items(&package, &query, &name_map));
            items.extend(build_skintone_group_items(&package, &query, &name_map));
            items.extend(build_face_group_items(&package, &query, &name_map));
            items.extend(build_clip_group_items(&package, &query, &name_map));
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

        check_cancelled()?;
        emit_package_progress(
            app, started, "Preparing relationship metadata",
            "Keeping large-package loading responsive.", 3, 5, package.files.len(), mmap.len(),
        );
        let relations = if merge_manifest.is_some()
            && mmap.len() > MAX_RELATION_ANALYSIS_PACKAGE_BYTES
        {
            // A manifest-backed 1 GiB merge already has authoritative source
            // ownership. Do not launch an uncancellable 1 GiB hashing and
            // decompression thread that competes with removal for CPU/RAM/I/O.
            eprintln!("[package-relations] skip optional large manifest analysis");
            BTreeMap::new()
        } else if mmap.len() <= MAX_RELATION_ANALYSIS_PACKAGE_BYTES {
            let relations = build_resource_relations(&package, &name_map, &items);
            check_cancelled()?;
            relations
        } else if let Some(relation_app) = app.cloned() {
            // Relationship detection hashes/decompresses payloads. For large
            // packages, compute optional UI badges after returning the list.
            eprintln!(
                "[package-scan] deferring relationship analysis: package={} bytes limit={} bytes",
                mmap.len(), MAX_RELATION_ANALYSIS_PACKAGE_BYTES
            );
            let relation_path = package_path.to_string();
            let relation_name_map = name_map.clone();
            let relation_items = items.clone();
            std::thread::spawn(move || {
                let result = (|| -> Result<BTreeMap<String, ResourceRelation>, String> {
                    let mmap = open_package(&relation_path)?;
                    let package = parse_package(&mmap)?;
                    Ok(build_resource_relations(
                        &package,
                        &relation_name_map,
                        &relation_items,
                    ))
                })();
                match result {
                    Ok(relations) => {
                        let _ = relation_app.emit(
                            "package-relations",
                            PackageRelationUpdate {
                                package_path: relation_path,
                                relations,
                            },
                        );
                    }
                    Err(error) => eprintln!(
                        "[package-relations] deferred analysis failed: {}",
                        error
                    ),
                }
            });
            BTreeMap::new()
        } else {
            // Unit tests do not own a Tauri AppHandle; calculate this result
            // synchronously so they still cover relation-analysis behavior.
            build_resource_relations(&package, &name_map, &items)
        };
        check_cancelled()?;
        emit_package_progress(
            app, started, "Ready", "Resource list ready.", 5, 5, package.files.len(), mmap.len(),
        );

        Ok(ScanResult {
            items,
            manifest_package_count,
            relations,
        })
    }))
    .map_err(|_| "Falha ao carregar a lista de recursos deste package.".to_string())?
}

#[tauri::command]
pub async fn analyze_remove_selection(
    app: AppHandle,
    payload: RemoveSelectionPayload,
) -> Result<RemoveSelectionPlan, String> {
    // Large-merged planning decodes metadata and traverses many resources.
    // Keep it off the Tauri command thread so the interface remains responsive.
    tauri::async_runtime::spawn_blocking(move || {
        analyze_remove_selection_inner(Some(&app), payload)
    })
    .await
    .map_err(|error| format!("Falha interna ao analisar a remoção: {error}"))?
}

fn analyze_remove_selection_inner(
    app: Option<&AppHandle>,
    payload: RemoveSelectionPayload,
) -> Result<RemoveSelectionPlan, String> {
    let started = Instant::now();
    trace_removal_stage("Análise iniciada", started, 2);
    emit_package_progress(
        app, started, "Analisando seleção",
        "Abrindo o merged sem bloquear a interface.",
        2, 100, 0, 0,
    );
    validate_package_extension(&payload.package_path)?;
    let mmap = open_package(&payload.package_path)?;
    let package = parse_package(&mmap)?;
    trace_removal_stage("Leitura de nomes iniciada", started, 35);
    emit_package_progress(
        app, started, "Lendo nomes",
        "Identificando metadados antes da análise de dependências.",
        35, 100, package.files.len(), mmap.len(),
    );
    let name_map = removal_name_map(&package, &payload.selected_keys);
    trace_removal_stage("Leitura de nomes concluída", started, 45);
    emit_package_progress(
        app, started, "Analisando dependências",
        "Verificando recursos compartilhados e seus vínculos.",
        45, 100, package.files.len(), mmap.len(),
    );
    trace_removal_stage("Grafo de dependências iniciado", started, 45);
    let plan = build_remove_selection_plan(&package, &payload.selected_keys, &name_map)?;
    trace_removal_stage("Grafo de dependências concluído", started, 95);
    trace_removal_stage("Análise concluída", started, 100);
    emit_package_progress(
        app, started, "Análise concluída",
        "Plano de remoção pronto para confirmação.",
        100, 100, package.files.len(), mmap.len(),
    );
    Ok(plan)
}

#[tauri::command]
pub async fn remove_selected_from_merged(
    app: AppHandle,
    payload: RemoveSelectionApplyPayload,
) -> Result<RemoveSelectionResult, String> {
    tauri::async_runtime::spawn_blocking(move || remove_selected_from_merged_inner(Some(&app), payload))
        .await
        .map_err(|error| format!("Falha interna ao remover do merged: {error}"))?
}

fn remove_selected_from_merged_inner(
    app: Option<&AppHandle>,
    payload: RemoveSelectionApplyPayload,
) -> Result<RemoveSelectionResult, String> {
    if payload.destination_folder.trim().is_empty() {
        return Err("Escolha uma pasta de destino.".to_string());
    }
    validate_package_extension(&payload.package_path)?;

    let started = Instant::now();
    let progress = |stage: &str, message: &str, percent: u32| {
        trace_removal_stage(stage, started, percent);
        emit_package_progress(app, started, stage, message, percent, 100, 0, 0);
    };
    progress("Analisando merged", "Preparando o arquivo e verificando os recursos.", 2);

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
    let name_map = removal_name_map(&package, &payload.selected_keys);
    let plan = build_remove_selection_plan(&package, &payload.selected_keys, &name_map)?;
    if plan.resources_to_remove == 0 && plan.manifest_packages_to_remove == 0 {
        return Err(
            "Nenhum recurso ou entrada de manifesto pode ser removido com segurança.".to_string(),
        );
    }

    let (_selected_resources, shared_preserved, resources_to_remove) =
        removal_plan_sets(&package, &payload.selected_keys, &name_map)?;

    progress("Preparando remoção", "Preservando dependências e recursos compartilhados.", 18);
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

    progress("Reconstruindo merged", "Gravando os recursos preservados em blocos de 4 MiB.", 25);
    if let Err(error) = write_removed_package(
        app,
        started,
        &package,
        &source_package,
        &mut source_file,
        &payload.selected_keys,
        &resources_to_remove,
        &temp_path,
    ) {
        let _ = remove_file(&temp_path);
        return Err(error);
    }

    progress("Validando merged", "Conferindo os recursos e o manifesto reconstruídos.", 66);
    if let Err(error) = validate_removed_package(
        &source_package,
        &temp_path,
        &payload.selected_keys,
        &resources_to_remove,
    ) {
        let _ = remove_file(&temp_path);
        return Err(error);
    }

    // Release every source-file handle BEFORE attempting the same-volume
    // rename. A lingering Windows memory map can hold the original open.
    drop(source_file);
    drop(source_package);
    drop(package);
    drop(mmap);

    if output_is_source {
        let mut cache = preview_cache()
            .lock()
            .map_err(|_| "Não foi possível liberar o cache de preview antes de substituir o merged.".to_string())?;
        if cache.as_ref().map(|entry| entry.path == source_canonical).unwrap_or(false) {
            *cache = None;
        }
    }

    let backup_dir = unique_backup_directory(&destination, stem);
    create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup_path = backup_dir.join(file_name);

    if output_is_source {
        // Rename the unmodified file into the backup directory on the same
        // volume. No 1 GiB backup copy, extra read or second original exists.
        // If installation fails, restore that very same original by rename.
        progress("Preservando original", "Movendo o merged intacto para backup no mesmo disco.", 88);
        if let Err(error) =
            replace_source_with_rollback(&source_path, &temp_path, &backup_path)
        {
            let _ = remove_file(&temp_path);
            return Err(error);
        }
    } else {
        // When the output is in another folder, preserve previous semantics:
        // the original stays in place and its copy is verified byte for byte.
        progress("Criando backup", "Copiando o original para o destino.", 75);
        copy_backup_with_progress(app, started, &source_path, &backup_path).map_err(|e| {
            let _ = remove_file(&temp_path);
            let _ = remove_file(&backup_path);
            format!("Não foi possível criar o backup do original: {e}")
        })?;
        progress("Verificando backup", "Conferindo o backup byte por byte.", 88);
        match files_are_identical(app, started, &source_path, &backup_path) {
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

        rename(&temp_path, &output_path).map_err(|e| {
            let _ = remove_file(&temp_path);
            format!("Não foi possível finalizar o novo merged: {e}")
        })?;
    }
    progress("Finalizando remoção", "Merged reconstruído e original preservado em backup.", 96);

    progress("Remoção concluída", "Merged atualizado com sucesso.", 100);
    Ok(RemoveSelectionResult {
        output_path: output_path.display().to_string(),
        backup_path: backup_path.display().to_string(),
        removed_resources: resources_to_remove.len(),
        removed_manifest_packages: plan.manifest_packages_to_remove,
        shared_resources_preserved: shared_preserved.len(),
        selected_resources_preserved: plan.selected_resources_preserved,
    })
}

#[tauri::command]
pub fn get_resource_preview(app: AppHandle, payload: PreviewPayload) -> Result<PreviewResult, String> {
    validate_package_extension(&payload.package_path)?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let path = canonical_package_path(&payload.package_path);
        let mut cache = preview_cache()
            .lock()
            .map_err(|_| "Cache de preview indisponível.".to_string())?;
        if cache
            .as_ref()
            .map(|entry| !preview_cache_is_current(entry, &path))
            .unwrap_or(true)
        {
            *cache = Some(build_preview_cache(&path)?);
        }
        let cached = cache
            .as_ref()
            .ok_or_else(|| "Cache de preview indisponível.".to_string())?;
        let package = &cached.package;

        for part in payload.resource_key.split("||") {
            let Ok(parts) = parse_key(part.trim()) else {
                continue;
            };
            if parts.resource_type != TYPE_XML {
                continue;
            }
            let Some(xml_index) = package.entry_index(
                parts.resource_type,
                parts.resource_group,
                parts.instance,
            ) else {
                continue;
            };
            let xml = &package.files[xml_index];
            if !is_pose_list_xml(xml) {
                continue;
            }

            let text = String::from_utf8_lossy(xml.data());
            let declared = xml_tag_values(&text, "Key").len();
            let matched = pose_list_clip_hashes(&text)
                .into_iter()
                .filter(|instance| {
                    package
                        .files
                        .iter()
                        .any(|entry| entry.resource_type == TYPE_CLIP && entry.instance == *instance)
                })
                .count();
            return Ok(PreviewResult {
                label: pose_list_display_label(&text, xml.instance),
                key: payload.resource_key.clone(),
                kind: "Pose".to_string(),
                details: format!("Pose List • {matched}/{declared} pose(s) vinculada(s)"),
                thumb_base64: pose_list_preview_base64(package, &text),
            });
        }

        let key = parse_key(first_compound_key(&payload.resource_key))?;
        let entry_index = package
            .entry_index(key.resource_type, key.resource_group, key.instance)
            .ok_or_else(|| "Resource not found in package.".to_string())?;
        let entry = &package.files[entry_index];
        let name_map = &cached.name_map;

        Ok(PreviewResult {
            label: object_display_label(package, entry, name_map),
            key: key_string(entry),
            kind: kind_from_type(entry.resource_type).to_string(),
            details: format!(
                "Type 0x{:08X} • Group 0x{:08X}",
                entry.resource_type, entry.resource_group
            ),
            thumb_base64: find_preview_base64(package, key)
                .or_else(|| external_thumbnail_base64(&app, key)),
        })
    }))
    .unwrap_or_else(|_| Err("A prévia deste recurso falhou inesperadamente; o arquivo original não foi alterado.".to_string()))
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
            format!(
                "{}.zip",
                selection_output_stem(&package, base_key, &name_map)
            )
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

    fn test_resource_key(resource_type: u32, group: u32, instance: u64) -> String {
        format!("0x{resource_type:08X}-0x{group:08X}-0x{instance:016X}")
    }

    #[test]
    fn object_export_uses_manifest_owned_icon_and_never_steals_another_objects_icon() {
        use crate::merge_manifest::LEGACY_MANIFEST_TYPE;

        for manifest_type in [MANIFEST_TYPE, LEGACY_MANIFEST_TYPE] {
            let path = test_path("two-object-icons-owned-by-manifest");
            let object_a = test_resource_key(TYPE_OBJD, 0, 0xA0A0);
            let object_b = test_resource_key(TYPE_OBJD, 0, 0xB0B0);
            let icon_a = test_resource_key(TYPE_ICON, 0, 0xC0C0);
            let icon_b = test_resource_key(TYPE_ICON, 0, 0xD0D0);
            let manifest = MergeManifest {
                version: 2,
                packages: vec![
                    ManifestPackage {
                        id: 1,
                        name: "Object A.package".to_string(),
                        original_size: 500,
                        anchors: vec![ManifestAnchor {kind: "OBJD".to_string(), key: object_a.clone()}],
                        resources: vec![
                            ManifestResource {key: object_a.clone()},
                            ManifestResource {key: icon_a.clone()},
                        ],
                        preserved_resources: vec![],
                    },
                    ManifestPackage {
                        id: 2,
                        name: "Object B.package".to_string(),
                        original_size: 500,
                        anchors: vec![ManifestAnchor {kind: "OBJD".to_string(), key: object_b.clone()}],
                        resources: vec![
                            ManifestResource {key: object_b.clone()},
                            ManifestResource {key: icon_b.clone()},
                        ],
                        preserved_resources: vec![],
                    },
                ],
            };
            write_test_package(&path, &[
                (TYPE_OBJD, 0, 0xA0A0, b"object-a".to_vec()),
                (TYPE_OBJD, 0, 0xB0B0, b"object-b".to_vec()),
                (TYPE_ICON, 0, 0xC0C0, b"icon-a".to_vec()),
                (TYPE_ICON, 0, 0xD0D0, b"icon-b".to_vec()),
                (manifest_type, MANIFEST_GROUP, MANIFEST_INSTANCE, manifest.to_xml_bytes()),
            ]);
            let mmap = open_package(&path.display().to_string()).unwrap();
            let package = parse_package(&mmap).unwrap();
            let keys = expand_selection_for_export(
                &package,
                &HashSet::from([object_a.clone()]),
                true,
                &BTreeMap::new(),
            ).unwrap();
            assert!(keys.contains(&object_a), "object must be retained");
            assert!(keys.contains(&icon_a), "correct ICON must be retained");
            assert!(!keys.contains(&icon_b), "unrelated ICON must not leak");
            assert!(!keys.contains(&object_b), "unrelated object must not leak");
            drop(package);
            drop(mmap);
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn uses_thumbnail_referenced_by_resource_in_same_package() {
        let path = test_path("embedded-casp-thumb");
        let casp_id = 0x1000_1234_5678_0001u64;
        let thumb_id = 0x9000_AAAA_BBBB_CCCCu64;
        let mut metadata = vec![42; 19];
        metadata.extend_from_slice(&thumb_id.to_le_bytes());
        metadata.extend_from_slice(&[1; 24]);
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[7; 45]);
        write_test_package(&path, &[
            (TYPE_CASP, 0, casp_id, metadata),
            (TYPE_THUM_LARGE, 0, thumb_id, png.clone()),
        ]);
        let mmap = open_package(&path.display().to_string()).unwrap();
        let package = parse_package(&mmap).unwrap();
        let key = ResourceKeyParts {
            resource_type: TYPE_CASP, resource_group: 0, instance: casp_id,
        };
        assert_eq!(STANDARD.decode(find_preview_base64(&package, key).unwrap()).unwrap(), png);
        drop(package);
        drop(mmap);
        let _ = remove_file(&path);
    }

    #[test]
    fn unrelated_thumbnail_is_never_used_as_resource_preview() {
        let path = test_path("unlinked-casp-thumb");
        let casp_id = 0x1111_1234_5678_0001u64;
        let thumb_id = 0xAAAA_1234_5678_0001u64;
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[7; 35]);
        write_test_package(&path, &[
            (TYPE_CASP, 0, casp_id, vec![1; 64]),
            (TYPE_THUM_LARGE, 0, thumb_id, png),
        ]);
        let mmap = open_package(&path.display().to_string()).unwrap();
        let package = parse_package(&mmap).unwrap();
        let key = ResourceKeyParts {
            resource_type: TYPE_CASP, resource_group: 0, instance: casp_id,
        };
        assert!(find_preview_base64(&package, key).is_none());
        drop(package);
        drop(mmap);
        let _ = remove_file(&path);
    }

    #[test]
    fn external_thumbnail_cache_matches_instance_and_prefers_expected_cache() {
        let root = test_dir("external-thumbnail-cache");
        let thumbnails = root.join("Thumbnails");
        std::fs::create_dir_all(&thumbnails).unwrap();

        let instance = 0x1234_5678_9ABC_DEF0u64;
        let mut cas_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        cas_png.extend_from_slice(&[1; 32]);
        let mut object_png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        object_png.extend_from_slice(&[2; 64]);

        write_test_package(
            &thumbnails.join("CASThumbnails.package"),
            &[(TYPE_THUM_LARGE, 1, instance, cas_png.clone())],
        );
        write_test_package(
            &thumbnails.join("ObjectThumbnails.package"),
            &[(0xAD366F96, 1, instance, object_png.clone())],
        );

        let cas_key = ResourceKeyParts {
            resource_type: TYPE_CASP,
            resource_group: 0,
            instance,
        };
        let object_key = ResourceKeyParts {
            resource_type: TYPE_OBJD,
            resource_group: 0,
            instance,
        };

        let cas_found = external_thumbnail_base64_from_dir(&thumbnails, cas_key).unwrap();
        let object_found = external_thumbnail_base64_from_dir(&thumbnails, object_key).unwrap();

        assert_eq!(STANDARD.decode(cas_found).unwrap(), cas_png);
        assert_eq!(STANDARD.decode(object_found).unwrap(), object_png);

        let missing = ResourceKeyParts {
            resource_type: TYPE_CASP,
            resource_group: 0,
            instance: instance + 1,
        };
        assert!(external_thumbnail_base64_from_dir(&thumbnails, missing).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }


    #[test]
    fn indexed_instance_binary_scan_matches_brute_force_reference() {
        let source = test_path("indexed-instance-scan");
        let instances = [
            0x1234_5678_ABC0_0001u64,
            0x1234_5678_ABC0_0002u64,
            0x1234_5678_ABC0_0003u64,
        ];
        write_test_package(
            &source,
            &[
                (TYPE_IMG, 0, instances[0], vec![5; 20]),
                (TYPE_IMG, 1, instances[0], vec![6; 20]),
                (TYPE_IMG, 0, instances[1], vec![7; 20]),
                (TYPE_XML, 0, instances[1], vec![8; 20]),
                (TYPE_IMG, 0, instances[2], vec![9; 20]),
            ],
        );
        let mmap = open_package(&source.display().to_string()).unwrap();
        let package = parse_package(&mmap).unwrap();
        let mut bytes = vec![0xA5; 33];
        bytes.extend_from_slice(&instances[0].to_le_bytes());
        bytes.extend_from_slice(&[0x4D; 3]);
        bytes.extend_from_slice(&instances[1].to_le_bytes());
        bytes.extend_from_slice(&instances[0].to_le_bytes());
        bytes.extend_from_slice(&[0x81; 17]);

        let mut expected = Vec::new();
        let mut seen = HashSet::new();
        for entry in package.files.iter().filter(|e| e.resource_type == TYPE_IMG) {
            if bytes.windows(8).any(|w| w == entry.instance.to_le_bytes()) {
                let parts = ResourceKeyParts {
                    resource_type: entry.resource_type,
                    resource_group: entry.resource_group,
                    instance: entry.instance,
                };
                if seen.insert(parts) {
                    expected.push(parts);
                }
            }
        }
        let actual = scan_existing_instances_from_bytes(&package, &bytes, &[TYPE_IMG]);
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 3, "Both groups and both instance matches must survive");
        let _ = remove_file(&source);
    }

    #[test]
    fn non_manifest_large_index_removes_five_but_retains_shared_dependencies() {
        let source = test_path("generic-large-index");
        let selected_base = 0x0001_0000_0000_1000u64;
        let shared = 0xF123_0000_0000_1111u64;
        let reference = format!("key:{TYPE_IMG:08X}:00000000:{shared:016X}");
        let mut resources = Vec::new();

        // Many independent logical resources, without any Veiga manifest.
        // Previous O(N²) companion/byte scans stalled before confirmation.
        for i in 0..2400u64 {
            let payload = if i == 0 || i == 5 {
                format!("<entry>{reference}</entry>").into_bytes()
            } else {
                format!("<entry index=\"{i}\"/>").into_bytes()
            };
            resources.push((TYPE_XML, 0, selected_base + i, payload));
        }
        resources.push((TYPE_IMG, 0, shared, vec![22; 64]));
        write_test_package(&source, &resources);
        let selected_keys: Vec<String> = (0..5u64)
            .map(|i| test_resource_key(TYPE_XML, 0, selected_base + i))
            .collect();
        let plan = analyze_remove_selection_inner(
            None,
            RemoveSelectionPayload {
                package_path: source.display().to_string(),
                selected_keys: selected_keys.clone(),
            },
        ).unwrap();

        assert_eq!(plan.manifest_package_total, 0);
        assert_eq!(plan.resources_to_remove, 5);
        assert_eq!(plan.shared_resources_preserved, 1);
        let removed = remove_selected_from_merged_inner(
            None,
            RemoveSelectionApplyPayload {
                package_path: source.display().to_string(),
                destination_folder: source.parent().unwrap().display().to_string(),
                selected_keys: selected_keys.clone(),
            },
        ).unwrap();
        assert_eq!(removed.removed_resources, 5);
        let rebuilt = WritablePackage::load(&removed.output_path).unwrap();
        assert_eq!(rebuilt.entries.len(), 2396);
        assert!(rebuilt.entries.iter().any(|e| e.type_id == TYPE_IMG && e.instance == shared));
        assert!(std::fs::metadata(&removed.backup_path).unwrap().len() > 0);
        drop(rebuilt);
        let _ = remove_file(&source);
        let _ = remove_file(&removed.output_path);
        let _ = remove_file(&removed.backup_path);
        let _ = std::fs::remove_dir_all(Path::new(&removed.backup_path).parent().unwrap());
    }

    #[test]
    fn preserves_shared_dependency_and_rebuilds_valid_package() {
        let source = test_path("remove-source");
        let rebuilt = test_path("remove-rebuilt");

        let shared_instance = 0x1111_2222_3333_4444u64;
        let selected_xml_instance = 0xAAAA_BBBB_CCCC_D001u64;
        let remaining_xml_instance = 0xAAAA_BBBB_CCCC_D002u64;
        let shared_key_text = format!("key:{TYPE_IMG:08X}:{:08X}:{shared_instance:016X}", 0u32);

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

        let shared_img_key = format!("0x{TYPE_IMG:08X}-0x{:08X}-0x{shared_instance:016X}", 0u32);
        assert!(remove.contains(&selected_key));
        assert!(shared.contains(&shared_img_key));
        assert!(!remove.contains(&shared_img_key));

        let source_package = WritablePackage::load(&source).unwrap();
        let mut source_file = File::open(&source).unwrap();
        write_removed_package(
            None,
            Instant::now(),
            &package,
            &source_package,
            &mut source_file,
            &[selected_key.clone()],
            &remove,
            &rebuilt,
        )
        .unwrap();
        validate_removed_package(&source_package, &rebuilt, &[selected_key.clone()], &remove)
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

    fn relation_for<'a>(
        scan: &'a ScanResult,
        key: &str,
    ) -> ResourceRelation {
        scan.relations.get(key).cloned().unwrap_or_default()
    }

    #[test]
    fn same_name_and_shared_dependency_do_not_make_different_items_duplicates() {
        let root = test_dir("same-name-shared-dependency");
        let path = root.join("SameNameSharedDependency.package");
        let first = 0xAAAA_2000_0000_0001u64;
        let second = 0xBBBB_2000_0000_0002u64;
        let shared = 0xCCCC_2000_0000_0003u64;
        let shared_key_text = format!("key:{TYPE_IMG:08X}:{:08X}:{shared:016X}", 0u32);

        write_test_package(
            &path,
            &[
                (
                    TYPE_XML,
                    0,
                    first,
                    format!("<first>{shared_key_text}</first>").into_bytes(),
                ),
                (
                    TYPE_XML,
                    0,
                    second,
                    format!("<second>{shared_key_text}</second>").into_bytes(),
                ),
                (TYPE_IMG, 0, shared, vec![7, 7, 7, 7]),
            ],
        );

        let mmap = open_package(&path.display().to_string()).unwrap();
        let package = parse_package(&mmap).unwrap();
        let name_map = BTreeMap::new();
        let items = vec![
            ResourceItem {
                label: "Same Internal Name".to_string(),
                key: test_resource_key(TYPE_XML, 0, first),
                kind: "XML".to_string(),
                details: String::new(),
                content_categories: Vec::new(),
            },
            ResourceItem {
                label: "Same Internal Name".to_string(),
                key: test_resource_key(TYPE_XML, 0, second),
                kind: "XML".to_string(),
                details: String::new(),
                content_categories: Vec::new(),
            },
        ];

        let relations = build_resource_relations(&package, &name_map, &items);
        assert!(relations.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_marks_exact_logical_content_as_duplicate_even_with_different_tgi() {
        let root = test_dir("duplicate-content");
        let path = root.join("DuplicateContent.package");
        let first = 0xAAAA_0000_0000_0001u64;
        let second = 0xBBBB_0000_0000_0002u64;
        let payload = vec![1, 2, 3, 4, 5, 6];

        write_test_package(
            &path,
            &[
                (TYPE_CASP, 0, first, payload.clone()),
                (TYPE_CASP, 0, second, payload),
            ],
        );

        let scan = scan_package_resources_inner(None, ScanPayload {
            package_path: path.display().to_string(),
            resource_type: "casp".to_string(),
            search: None,
        })
        .unwrap();

        assert_eq!(scan.items.len(), 2);
        assert_ne!(scan.items[0].label, scan.items[1].label);
        for item in &scan.items {
            let relation = relation_for(&scan, &item.key);
            assert_eq!(relation.duplicate_count, 2);
            assert_eq!(relation.tgi_conflict_count, 0);
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_does_not_mark_different_payloads_as_duplicate() {
        let root = test_dir("different-content");
        let path = root.join("DifferentContent.package");
        let first = 0xAAAA_1000_0000_0001u64;
        let second = 0xBBBB_1000_0000_0002u64;

        write_test_package(
            &path,
            &[
                (TYPE_CASP, 0, first, vec![1, 2, 3]),
                (TYPE_CASP, 0, second, vec![1, 2, 4]),
            ],
        );

        let scan = scan_package_resources_inner(None, ScanPayload {
            package_path: path.display().to_string(),
            resource_type: "casp".to_string(),
            search: None,
        })
        .unwrap();

        assert_eq!(scan.items.len(), 2);
        assert!(scan.relations.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_reports_tgi_conflict_when_same_tgi_has_different_payloads() {
        let root = test_dir("tgi-conflict");
        let path = root.join("TgiConflict.package");
        let instance = 0xCCCC_0000_0000_0003u64;

        write_test_package(
            &path,
            &[
                (TYPE_CASP, 0, instance, vec![1, 2, 3]),
                (TYPE_CASP, 0, instance, vec![9, 8, 7]),
            ],
        );

        let scan = scan_package_resources_inner(None, ScanPayload {
            package_path: path.display().to_string(),
            resource_type: "casp".to_string(),
            search: None,
        })
        .unwrap();

        assert_eq!(scan.items.len(), 2);
        let key = scan.items[0].key.clone();
        let relation = relation_for(&scan, &key);
        assert_eq!(relation.duplicate_count, 0);
        assert_eq!(relation.tgi_conflict_count, 2);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_marks_repeated_identical_tgi_as_duplicate_not_conflict() {
        let root = test_dir("repeated-identical-tgi");
        let path = root.join("RepeatedIdenticalTgi.package");
        let instance = 0xDDDD_0000_0000_0004u64;
        let payload = vec![4, 4, 4, 4];

        write_test_package(
            &path,
            &[
                (TYPE_CASP, 0, instance, payload.clone()),
                (TYPE_CASP, 0, instance, payload),
            ],
        );

        let scan = scan_package_resources_inner(None, ScanPayload {
            package_path: path.display().to_string(),
            resource_type: "casp".to_string(),
            search: None,
        })
        .unwrap();

        assert_eq!(scan.items.len(), 2);
        let key = scan.items[0].key.clone();
        let relation = relation_for(&scan, &key);
        assert_eq!(relation.duplicate_count, 2);
        assert_eq!(relation.tgi_conflict_count, 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removal_releases_preview_cache_and_preserves_verified_backup() {
        let root = test_dir("remove-with-preview-cache");
        let merged = root.join("Merged.package");
        let removed_instance = 0x1000_0000_0000_0001u64;
        let retained_instance = 0x1000_0000_0000_0002u64;
        write_test_package(
            &merged,
            &[
                (TYPE_IMG, 0, removed_instance, vec![1, 2, 3, 4]),
                (TYPE_IMG, 0, retained_instance, vec![5, 6, 7, 8]),
            ],
        );
        let before = std::fs::read(&merged).unwrap();
        let canonical = merged.canonicalize().unwrap();
        {
            let mut cache = preview_cache().lock().unwrap();
            *cache = Some(build_preview_cache(&canonical).unwrap());
        }

        let result = remove_selected_from_merged_inner(
            None,
            RemoveSelectionApplyPayload {
                package_path: merged.display().to_string(),
                destination_folder: root.display().to_string(),
                selected_keys: vec![test_resource_key(TYPE_IMG, 0, removed_instance)],
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(&result.backup_path).unwrap(), before);
        assert_eq!(result.removed_resources, 1);
        let rebuilt = WritablePackage::load(&merged).unwrap();
        assert_eq!(rebuilt.entries.len(), 1);
        assert_eq!(rebuilt.entries[0].instance, retained_instance);
        assert!(!preview_cache().lock().unwrap().as_ref()
            .map(|entry| entry.path == canonical).unwrap_or(false));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_only_marks_valid_veiga_merge_manifest_as_reversible() {
        let root = test_dir("manifest-detection");
        let valid = root.join("Valid.package");
        let common = root.join("Common.package");
        let other_tool = root.join("OtherToolMerged.package");
        let old = root.join("OldManifest.package");
        let corrupt = root.join("CorruptManifest.package");

        let casp = 0x1010_0000_0000_0001u64;
        let objd = 0x1010_0000_0000_0002u64;
        let casp_key = test_resource_key(TYPE_CASP, 0, casp);
        let valid_manifest = MergeManifest {
            version: 2,
            packages: vec![ManifestPackage {
                id: 1,
                name: "ValidOriginal.package".to_string(),
                original_size: 3,
                anchors: vec![ManifestAnchor {
                    kind: "CASP".to_string(),
                    key: casp_key.clone(),
                }],
                resources: vec![ManifestResource {
                    key: casp_key.clone(),
                }],
                preserved_resources: Vec::new(),
            }],
        };
        write_test_package(
            &valid,
            &[
                (TYPE_CASP, 0, casp, vec![1, 2, 3]),
                (
                    MANIFEST_TYPE,
                    MANIFEST_GROUP,
                    MANIFEST_INSTANCE,
                    valid_manifest.to_xml_bytes(),
                ),
            ],
        );

        write_test_package(&common, &[(TYPE_CASP, 0, casp, vec![1, 2, 3])]);
        write_test_package(
            &other_tool,
            &[
                (TYPE_CASP, 0, casp, vec![1, 2, 3]),
                (TYPE_OBJD, 0, objd, vec![4]),
            ],
        );

        let old_manifest = MergeManifest {
            version: 1,
            packages: vec![ManifestPackage {
                id: 1,
                name: "OldOriginal.package".to_string(),
                original_size: 3,
                anchors: vec![ManifestAnchor {
                    kind: "CASP".to_string(),
                    key: casp_key.clone(),
                }],
                resources: vec![ManifestResource {
                    key: casp_key.clone(),
                }],
                preserved_resources: Vec::new(),
            }],
        };
        write_test_package(
            &old,
            &[
                (TYPE_CASP, 0, casp, vec![1, 2, 3]),
                (
                    MANIFEST_TYPE,
                    MANIFEST_GROUP,
                    MANIFEST_INSTANCE,
                    old_manifest.to_xml_bytes(),
                ),
            ],
        );
        write_test_package(
            &corrupt,
            &[
                (TYPE_CASP, 0, casp, vec![1, 2, 3]),
                (
                    MANIFEST_TYPE,
                    MANIFEST_GROUP,
                    MANIFEST_INSTANCE,
                    b"not a veiga manifest".to_vec(),
                ),
            ],
        );

        for (path, expected_count) in [
            (&valid, 1usize),
            (&common, 0usize),
            (&other_tool, 0usize),
            (&old, 1usize),
            (&corrupt, 0usize),
        ] {
            let scan = scan_package_resources_inner(None, ScanPayload {
                package_path: path.display().to_string(),
                resource_type: "all".to_string(),
                search: None,
            })
            .unwrap();
            assert_eq!(scan.manifest_package_count, expected_count);
        }

        let valid_plan = analyze_remove_selection_inner(None, RemoveSelectionPayload {
            package_path: valid.display().to_string(),
            selected_keys: vec![casp_key.clone()],
        })
        .unwrap();
        assert_eq!(valid_plan.manifest_package_total, 1);
        assert_eq!(valid_plan.manifest_packages_to_remove, 1);

        for path in [&common, &other_tool, &corrupt] {
            let generic_plan = analyze_remove_selection_inner(None, RemoveSelectionPayload {
                package_path: path.display().to_string(),
                selected_keys: vec![casp_key.clone()],
            })
            .unwrap();
            assert_eq!(generic_plan.manifest_package_total, 0);
            assert_eq!(generic_plan.manifest_packages_to_remove, 0);
            assert!(generic_plan.resources_to_remove > 0);
        }

        let external_dir = root.join("external-output");
        std::fs::create_dir_all(&external_dir).unwrap();
        let external_result = remove_selected_from_merged_inner(None, RemoveSelectionApplyPayload {
            package_path: other_tool.display().to_string(),
            destination_folder: external_dir.display().to_string(),
            selected_keys: vec![casp_key.clone()],
        })
        .unwrap();
        assert_eq!(external_result.removed_manifest_packages, 0);
        assert!(!external_result.backup_path.is_empty());
        assert!(Path::new(&external_result.backup_path).is_file());
        assert!(Path::new(&external_result.output_path).is_file());

        let _ = std::fs::remove_dir_all(&root);
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
            allow_overwrite: false,
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

        let scan = scan_package_resources_inner(None, ScanPayload {
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
    fn merged_index_is_sorted_and_game_manifest_is_not_tuning_xml() {
        let root = test_dir("sorted-opaque-merge");
        let a = root.join("A.package");
        let b = root.join("B.package");
        let out = root.join("out");
        std::fs::create_dir_all(&out).unwrap();
        // Deliberately place high TGI before low TGI in the source.
        write_test_package(&a, &[
            (TYPE_STBL, 0, 0x9999, b"Catalog name".to_vec()),
            (TYPE_OBJD, 0, 0x8888, b"objd".to_vec()),
        ]);
        write_test_package(&b, &[(TYPE_CASP, 0, 0x7777, b"casp".to_vec())]);

        let merged = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![a.display().to_string(), b.display().to_string()],
            destination_folder: out.display().to_string(),
            output_name: "Merged.package".to_string(),
            allow_overwrite: false,
        }).unwrap();
        let result = WritablePackage::load(&merged.output_path).unwrap();
        assert!(result.entries.windows(2).all(|pair| {
            (pair[0].type_id, pair[0].group, pair[0].instance)
                <= (pair[1].type_id, pair[1].group, pair[1].instance)
        }));
        let manifest_entry = result.entries.iter()
            .find(|e| is_manifest_tgi(e.type_id, e.group, e.instance))
            .unwrap();
        assert_eq!(manifest_entry.type_id, MANIFEST_TYPE);
        assert_ne!(manifest_entry.type_id, TYPE_XML);
        for (ty, instance, expected) in [
            (TYPE_STBL, 0x9999, b"Catalog name".as_slice()),
            (TYPE_OBJD, 0x8888, b"objd".as_slice()),
            (TYPE_CASP, 0x7777, b"casp".as_slice()),
        ] {
            let entry = result.entries.iter()
                .find(|e| e.type_id == ty && e.instance == instance).unwrap();
            assert_eq!(result.get_uncompressed_data(entry).unwrap(), expected);
        }
        drop(result);
        let _ = std::fs::remove_dir_all(root);
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
        write_test_package(&source_b, &[(TYPE_IMG, 0, instance, vec![9, 9, 9, 9])]);

        let error = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: destination.display().to_string(),
            output_name: "Conflict.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap_err();

        assert!(error.contains("Conflito de TGI"));
        assert!(error.contains(&source_a.display().to_string()));
        assert!(error.contains(&source_b.display().to_string()));
        assert!(error.contains("IMG"));
        assert!(error.contains("3 bytes"));
        assert!(error.contains("4 bytes"));
        assert!(!destination.join("Conflict.package").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reversible_merger_preserves_package_manifest_and_key_metadata_per_source() {
        let root = test_dir("merge-source-metadata");
        let source_a = root.join("Eyebrows N01.package");
        let source_b = root.join("Eyebrows N02.package");
        let destination = root.join("out");
        let restore_dir = root.join("restored");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::create_dir_all(&restore_dir).unwrap();

        let casp_a = 0x7777_0000_0000_0001u64;
        let casp_b = 0x7777_0000_0000_0002u64;
        let package_manifest_a = b"<packageManifest name=\"N01\" />".to_vec();
        let package_manifest_b = b"<packageManifest name=\"N02\" />".to_vec();
        let key_a = b"KEY-N01".to_vec();
        let key_b = b"KEY-N02".to_vec();

        write_test_package(
            &source_a,
            &[
                (TYPE_CASP, 0, casp_a, vec![1, 2, 3]),
                (TYPE_PACKAGE_MANIFEST, 0, 0, package_manifest_a.clone()),
                (TYPE_KEY, 0, 0, key_a.clone()),
            ],
        );
        write_test_package(
            &source_b,
            &[
                (TYPE_CASP, 0, casp_b, vec![4, 5, 6]),
                (TYPE_PACKAGE_MANIFEST, 0, 0, package_manifest_b.clone()),
                (TYPE_KEY, 0, 0, key_b.clone()),
            ],
        );

        let merged = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: destination.display().to_string(),
            output_name: "EyebrowsMerged.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let merged_package = WritablePackage::load(&merged.output_path).unwrap();
        assert!(!merged_package
            .entries
            .iter()
            .any(|entry| entry.type_id == TYPE_PACKAGE_MANIFEST));
        assert!(!merged_package
            .entries
            .iter()
            .any(|entry| entry.type_id == TYPE_KEY));

        let manifest = read_merge_manifest_from_writable(&merged_package)
            .unwrap()
            .unwrap();
        assert_eq!(manifest.version, 2);
        assert_eq!(manifest.packages.len(), 2);
        assert!(manifest.packages.iter().all(|package| {
            package
                .preserved_resources
                .iter()
                .any(|resource| resource.key.starts_with("0x73E93EEB-"))
        }));
        assert!(manifest.packages.iter().all(|package| {
            package
                .preserved_resources
                .iter()
                .any(|resource| resource.key.starts_with("0x0166038C-"))
        }));

        restore_manifest_packages(RestoreManifestPayload {
            package_path: merged.output_path.clone(),
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();

        let restored_a = WritablePackage::load(restore_dir.join("Eyebrows N01.package")).unwrap();
        let restored_b = WritablePackage::load(restore_dir.join("Eyebrows N02.package")).unwrap();

        let restored_manifest_a = restored_a
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_PACKAGE_MANIFEST)
            .unwrap();
        let restored_manifest_b = restored_b
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_PACKAGE_MANIFEST)
            .unwrap();
        assert_eq!(
            restored_a
                .get_uncompressed_data(restored_manifest_a)
                .unwrap(),
            package_manifest_a
        );
        assert_eq!(
            restored_b
                .get_uncompressed_data(restored_manifest_b)
                .unwrap(),
            package_manifest_b
        );

        let restored_key_a = restored_a
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_KEY)
            .unwrap();
        let restored_key_b = restored_b
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_KEY)
            .unwrap();
        assert_eq!(
            restored_a.get_uncompressed_data(restored_key_a).unwrap(),
            key_a
        );
        assert_eq!(
            restored_b.get_uncompressed_data(restored_key_b).unwrap(),
            key_b
        );

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
            allow_overwrite: false,
        })
        .unwrap();

        let rebuilt = root.join("rebuilt.package");
        let mmap = open_package(&merged.output_path).unwrap();
        let dbpf = parse_package(&mmap).unwrap();
        let source_package = WritablePackage::load(&merged.output_path).unwrap();
        let mut source_file = File::open(&merged.output_path).unwrap();
        let selected_key = format!("0x{TYPE_CASP:08X}-0x00000000-0x{casp_a:016X}");
        let name_map = removal_name_map(&dbpf, &[selected_key.clone()]);
        assert!(name_map.is_empty(), "manifest removal must not decode all resource names");
        let (_selected_resources, shared_resources, removed) =
            removal_plan_sets(&dbpf, &[selected_key.clone()], &name_map).unwrap();
        let shared_key = format!("0x{TYPE_IMG:08X}-0x00000000-0x{shared:016X}");
        assert!(removed.contains(&selected_key));
        assert!(!removed.contains(&shared_key));
        assert!(shared_resources.contains(&shared_key));

        write_removed_package(
            None,
            Instant::now(),
            &dbpf,
            &source_package,
            &mut source_file,
            &[selected_key.clone()],
            &removed,
            &rebuilt,
        )
        .unwrap();
        validate_removed_package(&source_package, &rebuilt, &[selected_key], &removed).unwrap();

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

    #[test]
    fn atomic_merged_swap_moves_exact_original_to_backup() {
        let root = test_dir("atomic-merged-backup");
        let source = root.join("Merged.package");
        let temp = root.join(".Merged.remove-temp.package");
        let backup = root.join("Merged_backup").join("Merged.package");
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        let original = b"intact-original-merged".to_vec();
        let rebuilt = b"validated-rebuilt-merged".to_vec();
        std::fs::write(&source, &original).unwrap();
        std::fs::write(&temp, &rebuilt).unwrap();

        replace_source_with_rollback(&source, &temp, &backup).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), rebuilt);
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert!(!temp.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn merger_requires_explicit_consent_and_preserves_replaced_output_in_a_backup() {
        let root = test_dir("merger-safe-overwrite");
        let source_a = root.join("Source_A.package");
        let source_b = root.join("Source_B.package");
        let destination = root.join("output");
        std::fs::create_dir_all(&destination).unwrap();
        write_test_package(&source_a, &[(TYPE_CASP, 0, 0xA1123, vec![1, 2, 3])]);
        write_test_package(&source_b, &[(TYPE_CASP, 0, 0xB1123, vec![4, 5, 6])]);

        let original_a = std::fs::read(&source_a).unwrap();
        let original_b = std::fs::read(&source_b).unwrap();
        // A legacy temp path may be a legitimate user file. It must survive.
        let existing_legacy_temp =
            destination.join(format!(".Merged.merge-temp-{}.package", std::process::id()));
        std::fs::write(&existing_legacy_temp, b"legacy-temp-must-not-be-deleted").unwrap();
        let output = destination.join("Merged.package");
        std::fs::write(&output, b"old-merged-kept-exactly").unwrap();

        let mut payload = MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: destination.display().to_string(),
            output_name: "Merged.package".to_string(),
            allow_overwrite: false,
        };
        let error = merge_packages_with_manifest(payload.clone()).unwrap_err();
        assert!(error.contains("Já existe"), "{error}");
        assert_eq!(std::fs::read(&output).unwrap(), b"old-merged-kept-exactly");

        payload.allow_overwrite = true;
        let result = merge_packages_with_manifest(payload.clone()).unwrap();
        let saved = result.backup_path.expect("A confirmed replacement must create a backup");
        assert!(saved.ends_with(".s3ccbak"));
        assert_eq!(std::fs::read(&saved).unwrap(), b"old-merged-kept-exactly");
        let first_merged = std::fs::read(&output).unwrap();
        assert!(WritablePackage::load(&output).is_ok());

        let second = merge_packages_with_manifest(payload).unwrap();
        let backup_again = second.backup_path.expect("Each overwrite needs another backup");
        assert_ne!(saved, backup_again);
        assert_eq!(std::fs::read(&backup_again).unwrap(), first_merged);
        assert_eq!(std::fs::read(&source_a).unwrap(), original_a);
        assert_eq!(std::fs::read(&source_b).unwrap(), original_b);
        assert_eq!(
            std::fs::read(&existing_legacy_temp).unwrap(),
            b"legacy-temp-must-not-be-deleted"
        );

        // No replacement of an original source is allowed, even with consent.
        let protected = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: root.display().to_string(),
            output_name: "Source_A.package".to_string(),
            allow_overwrite: true,
        }).unwrap_err();
        assert!(protected.contains("package de origem"), "{protected}");
        assert_eq!(std::fs::read(&source_a).unwrap(), original_a);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_merged_swap_restores_original_when_rebuilt_missing() {
        let root = test_dir("atomic-merged-rollback");
        let source = root.join("Merged.package");
        let temp = root.join(".nonexistent.remove-temp.package");
        let backup = root.join("Merged_backup").join("Merged.package");
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        let original = b"untouched-original".to_vec();
        std::fs::write(&source, &original).unwrap();

        let error = replace_source_with_rollback(&source, &temp, &backup).unwrap_err();
        assert!(error.contains("restaurado"), "{error}");
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert!(!backup.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn atomic_merged_swap_does_not_overwrite_existing_backup() {
        let root = test_dir("atomic-merged-existing-backup");
        let source = root.join("Merged.package");
        let temp = root.join(".Merged.remove-temp.package");
        let backup = root.join("Merged_backup").join("Merged.package");
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(&temp, b"rebuilt").unwrap();
        std::fs::write(&backup, b"prior-backup").unwrap();

        assert!(replace_source_with_rollback(&source, &temp, &backup).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert_eq!(std::fs::read(&temp).unwrap(), b"rebuilt");
        assert_eq!(std::fs::read(&backup).unwrap(), b"prior-backup");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removing_five_sources_streams_multi_chunk_merged_and_preserves_backup() {
        let root = test_dir("five-sources-large-merged");
        let destination = root.join("merged");
        std::fs::create_dir_all(&destination).unwrap();

        // Normal CI exercises the 4 MiB streaming boundary with 60 MiB.
        // Set S3CC_SPLITTER_TEST_1GB=1 to exercise the same flow with a
        // synthetic merged larger than 1 GiB (requires ~3 GiB free disk).
        let payload_size = if std::env::var_os("S3CC_SPLITTER_TEST_1GB").is_some() {
            105 * 1024 * 1024
        } else {
            6 * 1024 * 1024
        };
        let mut sources = Vec::new();
        let mut keys = Vec::new();

        for index in 0..10u64 {
            let path = root.join(format!("Source_{index:02}.package"));
            let instance = 0x7700_0000_0000_0000u64 + index;
            write_test_package(
                &path,
                &[(TYPE_IMG, 0, instance, vec![index as u8 + 1; payload_size])],
            );
            sources.push(path.display().to_string());
            keys.push(test_resource_key(TYPE_IMG, 0, instance));
        }

        let merged = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: sources,
            destination_folder: destination.display().to_string(),
            output_name: "Merged.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();
        let original_size = std::fs::metadata(&merged.output_path).unwrap().len();
        let selected_keys: Vec<String> = keys[..5].to_vec();

        let plan = analyze_remove_selection_inner(
            None,
            RemoveSelectionPayload {
                package_path: merged.output_path.clone(),
                selected_keys: selected_keys.clone(),
            },
        )
        .unwrap();
        assert_eq!(plan.manifest_package_total, 10);
        assert_eq!(plan.manifest_packages_to_remove, 5);

        let removal = remove_selected_from_merged_inner(
            None,
            RemoveSelectionApplyPayload {
                package_path: merged.output_path.clone(),
                destination_folder: destination.display().to_string(),
                selected_keys,
            },
        )
        .unwrap();

        assert_eq!(removal.removed_manifest_packages, 5);
        assert_eq!(removal.removed_resources, 5);
        assert_eq!(std::fs::metadata(&removal.backup_path).unwrap().len(), original_size);

        let rebuilt = WritablePackage::load(&removal.output_path).unwrap();
        let manifest = read_merge_manifest_from_writable(&rebuilt)
            .unwrap()
            .unwrap();
        assert_eq!(manifest.packages.len(), 5);
        for index in 0..10 {
            let present = rebuilt.entries.iter().any(|entry| {
                key_string_from_writable(entry) == keys[index]
            });
            assert_eq!(present, index >= 5, "Unexpected resource from source {index}");
        }

        let original_backup = WritablePackage::load(&removal.backup_path).unwrap();
        let backup_manifest = read_merge_manifest_from_writable(&original_backup)
            .unwrap()
            .unwrap();
        assert_eq!(backup_manifest.packages.len(), 10);

        drop(rebuilt);
        drop(original_backup);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn remove_selection_plan_reports_original_manifest_package_ratio() {
        let root = test_dir("manifest-removal-ratio");
        let source_dir = root.join("sources");
        let destination = root.join("out");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&destination).unwrap();

        let shared = 0x9999_0000_0000_1000u64;
        let mut paths = Vec::new();
        let mut casp_instances = Vec::new();
        for index in 0..10u64 {
            let casp = 0x9999_0000_0000_2000u64 + index;
            casp_instances.push(casp);
            let path = source_dir.join(format!("Original_{index}.package"));
            let mut resources = vec![(TYPE_CASP, 0, casp, vec![index as u8 + 1])];
            if index == 0 {
                resources.push((TYPE_IMG, 0, shared, vec![7, 7, 7]));
                resources.push((TYPE_OBJD, 0, 0x9999_0000_0000_3000, vec![3, 3, 3]));
            } else if index == 1 {
                resources.push((TYPE_IMG, 0, shared, vec![7, 7, 7]));
            }
            write_test_package(&path, &resources);
            paths.push(path.display().to_string());
        }

        let merged = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: paths,
            destination_folder: destination.display().to_string(),
            output_name: "MergedTen.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let mmap = open_package(&merged.output_path).unwrap();
        let dbpf = parse_package(&mmap).unwrap();
        let name_map = package_resource_names(&dbpf);
        let first = test_resource_key(TYPE_CASP, 0, casp_instances[0]);
        let second = test_resource_key(TYPE_CASP, 0, casp_instances[1]);
        let same_original_extra = test_resource_key(TYPE_OBJD, 0, 0x9999_0000_0000_3000);

        let two_of_ten =
            build_remove_selection_plan(&dbpf, &[first.clone(), second.clone()], &name_map)
                .unwrap();
        assert_eq!(two_of_ten.manifest_packages_to_remove, 2);
        assert_eq!(two_of_ten.manifest_package_total, 10);

        let one_of_ten =
            build_remove_selection_plan(&dbpf, std::slice::from_ref(&first), &name_map).unwrap();
        assert_eq!(one_of_ten.manifest_packages_to_remove, 1);
        assert_eq!(one_of_ten.manifest_package_total, 10);
        assert!(one_of_ten.shared_resources_preserved > 0);

        let same_original =
            build_remove_selection_plan(&dbpf, &[first.clone(), same_original_extra], &name_map)
                .unwrap();
        assert_eq!(same_original.manifest_packages_to_remove, 1);
        assert_eq!(same_original.manifest_package_total, 10);

        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn clip_and_pose_hashes_match_real_greatdaysimmer_samples() {
        assert_eq!(clip_name_hash("a_gds_salut"), 0x49D2_EBEE_3CB0_9B79);
        assert_eq!(
            clip_name_hash("a_GDS_Juniper_SelfiePose1"),
            0x2BC3_29D9_4915_B6C2
        );
        assert_eq!(fnv64_name_hash("1"), 0xAF63_BD4C_8601_B7EE);
        assert_eq!(
            fnv64_name_hash("a_GDS_ROSELIPA_pose_1"),
            0xCEC2_C5DF_F572_6C85
        );
    }

    #[test]
    fn scan_groups_pose_list_but_keeps_unidentified_clip_as_clip() {
        let root = test_dir("pose-list-clip-detection");
        let path = root.join("MixedPoseAndClip.package");

        let pose_code = "a_GDS_Juniper_SelfiePose1";
        let pose_instance = clip_name_hash(pose_code);
        let extra_clip = clip_name_hash("a_gds_not_in_pose_list");
        let icon_instance = fnv64_name_hash("1");
        let pose_list_instance = 0x0102_0304_0506_0708u64;
        let xml = format!(
            "<PoseList><Pose><Key>{pose_code}</Key><Name>Selfie 1</Name><Creator>Greatdaysimmer</Creator><IconKey>1</IconKey><Description>Selfie Poses by Juniper</Description></Pose></PoseList>"
        )
        .into_bytes();

        write_test_package(
            &path,
            &[
                (TYPE_CLIP, 0, pose_instance, vec![1, 2, 3]),
                (TYPE_XML, 0, pose_list_instance, xml),
                (TYPE_S3SA, 0, pose_list_instance, vec![4, 5, 6]),
                (TYPE_IMAG_PNG, 0, icon_instance, vec![7, 8, 9]),
                (TYPE_CLIP, 0, extra_clip, vec![10, 11, 12]),
            ],
        );

        let scan = scan_package_resources_inner(None, ScanPayload {
            package_path: path.display().to_string(),
            resource_type: "all".to_string(),
            search: None,
        })
        .unwrap();

        assert_eq!(scan.items.len(), 2);
        let pose = scan
            .items
            .iter()
            .find(|item| item.content_categories == vec!["Pose".to_string()])
            .expect("Pose List should be exposed as Pose");
        assert!(pose.label.contains("Selfie Poses by Juniper"));
        assert!(pose.key.contains(&format!("0x{TYPE_CLIP:08X}")));
        assert!(pose.key.contains(&format!("0x{TYPE_S3SA:08X}")));
        assert!(pose.key.contains(&format!("0x{TYPE_XML:08X}")));
        assert!(pose.key.contains(&format!("0x{TYPE_IMAG_PNG:08X}")));

        let clip = scan
            .items
            .iter()
            .find(|item| item.content_categories == vec!["Clip".to_string()])
            .expect("Unidentified CLIP should stay Clip");
        assert_eq!(clip.kind, "CLIP");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reversible_clip_merger_writes_consolidated_key_and_restores_source_keys() {
        let root = test_dir("reversible-clip-merge");
        let source_a = root.join("PoseOrAnimationA.package");
        let source_b = root.join("PoseOrAnimationB.package");
        let merged_dir = root.join("merged");
        let restore_dir = root.join("restored");
        std::fs::create_dir_all(&merged_dir).unwrap();
        std::fs::create_dir_all(&restore_dir).unwrap();

        let clip_a = clip_name_hash("a_gds_af_happyhug");
        let clip_b = clip_name_hash("a_gds_af_sadhug");

        let mut names_a = BTreeMap::new();
        names_a.insert(clip_a, "a_gds_af_happyhug".to_string());
        let key_a = build_key_data_from_names(&names_a);

        let mut names_b = BTreeMap::new();
        names_b.insert(clip_b, "a_gds_af_sadhug".to_string());
        let key_b = build_key_data_from_names(&names_b);

        write_test_package(
            &source_a,
            &[
                (TYPE_CLIP, 0, clip_a, vec![1, 2, 3]),
                (TYPE_KEY, 0, 0, key_a.clone()),
            ],
        );
        write_test_package(
            &source_b,
            &[
                (TYPE_CLIP, 0, clip_b, vec![4, 5, 6]),
                (TYPE_KEY, 0, 0, key_b.clone()),
            ],
        );

        let result = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: merged_dir.display().to_string(),
            output_name: "MergedClips.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let merged = WritablePackage::load(&result.output_path).unwrap();
        let key_entries: Vec<&WritableEntry> = merged
            .entries
            .iter()
            .filter(|entry| entry.type_id == TYPE_KEY)
            .collect();
        assert_eq!(key_entries.len(), 1);
        assert_eq!(key_entries[0].instance, 0);
        let merged_key_data = merged.get_uncompressed_data(key_entries[0]).unwrap();
        let merged_names = parse_key_names_data(&merged_key_data);
        assert_eq!(merged_names.get(&clip_a).map(String::as_str), Some("a_gds_af_happyhug"));
        assert_eq!(merged_names.get(&clip_b).map(String::as_str), Some("a_gds_af_sadhug"));

        let manifest = read_merge_manifest_from_writable(&merged).unwrap().unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert!(manifest
            .packages
            .iter()
            .all(|package| package.anchors.iter().all(|anchor| anchor.kind == "CLIP")));
        drop(merged);

        let restored = restore_manifest_packages(RestoreManifestPayload {
            package_path: result.output_path.clone(),
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();
        assert_eq!(restored.package_count, 2);

        let restored_a = WritablePackage::load(restore_dir.join("PoseOrAnimationA.package")).unwrap();
        let restored_a_key = restored_a
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_KEY)
            .unwrap();
        assert_eq!(restored_a.get_uncompressed_data(restored_a_key).unwrap(), key_a);

        let restored_b = WritablePackage::load(restore_dir.join("PoseOrAnimationB.package")).unwrap();
        let restored_b_key = restored_b
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_KEY)
            .unwrap();
        assert_eq!(restored_b.get_uncompressed_data(restored_b_key).unwrap(), key_b);

        let _ = std::fs::remove_dir_all(&root);
    }


    #[test]
    fn merger_combines_multiple_pose_lists_and_unmerge_restores_original_resources() {
        let root = test_dir("multiple-pose-list-combine");
        let source_a = root.join("PoseListA.package");
        let source_b = root.join("PoseListB.package");
        let merged_dir = root.join("merged");
        let restore_dir = root.join("restored");
        std::fs::create_dir_all(&merged_dir).unwrap();
        std::fs::create_dir_all(&restore_dir).unwrap();

        let clip_a = clip_name_hash("a_gds_pose_guard_a");
        let clip_b = clip_name_hash("a_gds_pose_guard_b");
        let xml_instance_a = 0x1111_0000_0000_0001u64;
        let xml_instance_b = 0x2222_0000_0000_0002u64;
        let xml_a = b"<?xml version=\"1.0\" encoding=\"utf-8\"?><PoseList><Pose><Key></Key><Name></Name><Creator></Creator><IconKey></IconKey><Description></Description></Pose><Pose><Key>a_gds_pose_guard_a</Key><Name>Pose A</Name><Creator>Test</Creator><IconKey>1</IconKey><Description>First set</Description></Pose></PoseList>".to_vec();
        let xml_b = b"<?xml version=\"1.0\" encoding=\"utf-8\"?><PoseList><Pose><Key></Key><Name></Name><Creator></Creator><IconKey></IconKey><Description></Description></Pose><Pose><Key>a_gds_pose_guard_b</Key><Name>Pose B</Name><Creator>Test</Creator><IconKey>2</IconKey><Description>Second set</Description></Pose></PoseList>".to_vec();
        let registrar_a = vec![4, 5, 6];
        let registrar_b = vec![10, 11, 12];

        write_test_package(
            &source_a,
            &[
                (TYPE_CLIP, 0, clip_a, vec![1, 2, 3]),
                (TYPE_XML, 0, xml_instance_a, xml_a.clone()),
                (TYPE_S3SA, 0, xml_instance_a, registrar_a.clone()),
            ],
        );
        write_test_package(
            &source_b,
            &[
                (TYPE_CLIP, 0, clip_b, vec![7, 8, 9]),
                (TYPE_XML, 0, xml_instance_b, xml_b.clone()),
                (TYPE_S3SA, 0, xml_instance_b, registrar_b.clone()),
            ],
        );

        let result = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                source_a.display().to_string(),
                source_b.display().to_string(),
            ],
            destination_folder: merged_dir.display().to_string(),
            output_name: "CombinedPoseLists.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let merged = WritablePackage::load(&result.output_path).unwrap();
        let pose_xmls: Vec<&WritableEntry> = merged
            .entries
            .iter()
            .filter(|entry| {
                if entry.type_id != TYPE_XML {
                    return false;
                }
                merged
                    .get_uncompressed_data(entry)
                    .map(|data| is_pose_list_text(&String::from_utf8_lossy(&data)))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(pose_xmls.len(), 1);
        let combined_xml = String::from_utf8(
            merged.get_uncompressed_data(pose_xmls[0]).unwrap(),
        )
        .unwrap();
        assert!(combined_xml.contains("a_gds_pose_guard_a"));
        assert!(combined_xml.contains("a_gds_pose_guard_b"));
        assert_eq!(combined_xml.matches("<Pose>").count(), 3);

        let registrars: Vec<&WritableEntry> = merged
            .entries
            .iter()
            .filter(|entry| entry.type_id == TYPE_S3SA)
            .collect();
        assert_eq!(registrars.len(), 1);
        assert_eq!(registrars[0].instance, xml_instance_a);

        let manifest = read_merge_manifest_from_writable(&merged).unwrap().unwrap();
        assert_eq!(manifest.packages.len(), 2);
        assert!(manifest.packages.iter().all(|package| {
            let instance = if package.name == "PoseListA.package" {
                xml_instance_a
            } else {
                xml_instance_b
            };
            package.preserved_resources.iter().any(|resource| {
                resource.key == test_resource_key(TYPE_XML, 0, instance)
            }) && package.preserved_resources.iter().any(|resource| {
                resource.key == test_resource_key(TYPE_S3SA, 0, instance)
            })
        }));

        let mmap = open_package(&result.output_path).unwrap();
        let dbpf = parse_package(&mmap).unwrap();
        let pose_selection = [
            test_resource_key(TYPE_CLIP, 0, clip_a),
            test_resource_key(TYPE_CLIP, 0, clip_b),
            test_resource_key(TYPE_XML, 0, xml_instance_a),
            test_resource_key(TYPE_S3SA, 0, xml_instance_a),
        ]
        .join("||");
        let (_, _, removal_keys) =
            removal_plan_sets(&dbpf, &[pose_selection], &BTreeMap::new()).unwrap();
        assert!(removal_keys.contains(&test_resource_key(TYPE_XML, 0, xml_instance_a)));
        assert!(removal_keys.contains(&test_resource_key(TYPE_S3SA, 0, xml_instance_a)));
        drop(dbpf);
        drop(mmap);
        drop(registrars);
        drop(pose_xmls);
        drop(manifest);
        drop(merged);

        let restored = restore_manifest_packages(RestoreManifestPayload {
            package_path: result.output_path.clone(),
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();
        assert_eq!(restored.package_count, 2);

        let restored_a = WritablePackage::load(restore_dir.join("PoseListA.package")).unwrap();
        let restored_b = WritablePackage::load(restore_dir.join("PoseListB.package")).unwrap();
        let restored_xml_a = restored_a
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_XML && entry.instance == xml_instance_a)
            .unwrap();
        let restored_xml_b = restored_b
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_XML && entry.instance == xml_instance_b)
            .unwrap();
        let restored_registrar_a = restored_a
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_S3SA && entry.instance == xml_instance_a)
            .unwrap();
        let restored_registrar_b = restored_b
            .entries
            .iter()
            .find(|entry| entry.type_id == TYPE_S3SA && entry.instance == xml_instance_b)
            .unwrap();

        assert_eq!(restored_a.get_uncompressed_data(restored_xml_a).unwrap(), xml_a);
        assert_eq!(restored_b.get_uncompressed_data(restored_xml_b).unwrap(), xml_b);
        assert_eq!(
            restored_a.get_uncompressed_data(restored_registrar_a).unwrap(),
            registrar_a
        );
        assert_eq!(
            restored_b.get_uncompressed_data(restored_registrar_b).unwrap(),
            registrar_b
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unmerge_skips_cross_merge_duplicates_and_renames_name_collisions() {
        let root = test_dir("unmerge-cross-merge-dedup");
        let shared = root.join("SharedCC.package");
        let first_only = root.join("FirstOnly.package");
        let second_only = root.join("SecondOnly.package");
        let merged_a_dir = root.join("merged-a");
        let merged_b_dir = root.join("merged-b");
        let restore_dir = root.join("restored");
        std::fs::create_dir_all(&merged_a_dir).unwrap();
        std::fs::create_dir_all(&merged_b_dir).unwrap();
        std::fs::create_dir_all(&restore_dir).unwrap();

        write_test_package(
            &shared,
            &[(TYPE_CASP, 0, 0x1111_0000_0000_0001, vec![1, 2, 3, 4])],
        );
        write_test_package(
            &first_only,
            &[(TYPE_CASP, 0, 0x2222_0000_0000_0002, vec![5, 6, 7, 8])],
        );
        write_test_package(
            &second_only,
            &[(TYPE_CASP, 0, 0x3333_0000_0000_0003, vec![9, 10, 11, 12])],
        );

        let merged_a = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                shared.display().to_string(),
                first_only.display().to_string(),
            ],
            destination_folder: merged_a_dir.display().to_string(),
            output_name: "MergedA.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let merged_b = merge_packages_with_manifest(MergePackagesPayload {
            package_paths: vec![
                shared.display().to_string(),
                second_only.display().to_string(),
            ],
            destination_folder: merged_b_dir.display().to_string(),
            output_name: "MergedB.package".to_string(),
            allow_overwrite: false,
        })
        .unwrap();

        let first_restore = restore_manifest_packages(RestoreManifestPayload {
            package_path: merged_a.output_path,
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();
        assert_eq!(first_restore.package_count, 2);
        assert_eq!(first_restore.duplicate_count, 0);
        assert!(restore_dir.join("SharedCC.package").exists());
        assert!(restore_dir.join("FirstOnly.package").exists());

        // Same filename, intentionally different content: this must not be
        // mistaken for a duplicate and must not be overwritten.
        write_test_package(
            &restore_dir.join("SecondOnly.package"),
            &[(TYPE_CASP, 0, 0x4444_0000_0000_0004, vec![90, 91, 92])],
        );

        let second_restore = restore_manifest_packages(RestoreManifestPayload {
            package_path: merged_b.output_path,
            destination_folder: restore_dir.display().to_string(),
        })
        .unwrap();

        assert_eq!(second_restore.package_count, 1);
        assert_eq!(second_restore.duplicate_count, 1);
        assert_eq!(second_restore.duplicate_paths.len(), 1);
        assert_eq!(
            PathBuf::from(&second_restore.duplicate_paths[0]),
            restore_dir.join("SharedCC.package")
        );
        assert_eq!(second_restore.output_paths.len(), 1);
        assert_eq!(
            PathBuf::from(&second_restore.output_paths[0]),
            restore_dir.join("SecondOnly (2).package")
        );
        assert!(restore_dir.join("SecondOnly.package").exists());
        assert!(restore_dir.join("SecondOnly (2).package").exists());
        assert!(!restore_dir.join("SharedCC (2).package").exists());

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
        assert_eq!(
            casp_content_category(Some(casp("Hair", Some("Hair"))), ""),
            "Hair"
        );
        assert_eq!(
            casp_content_category(Some(casp("Hair", Some("Beard"))), ""),
            "Facial Hair"
        );
        assert_eq!(
            casp_content_category(Some(casp("Hair", Some("Eyebrows"))), ""),
            "Eyebrows"
        );
        assert_eq!(
            casp_content_category(Some(casp("Hair", Some("Body Hair"))), ""),
            "Body Hair"
        );
        assert_eq!(
            casp_content_category(Some(casp("Accessories", Some("Rings"))), ""),
            "Accessories"
        );
        assert_eq!(
            casp_content_category(Some(casp("Clothing", None)), ""),
            "Clothes"
        );
        assert_eq!(casp_content_category(None, ""), "Other CASP");
    }

    #[test]
    fn classifies_special_casp_name_fallbacks() {
        assert_eq!(casp_content_category(None, "Creator_WinterBoot"), "Shoes");
        assert_eq!(casp_content_category(None, "CustomEye"), "Eyes");
        assert_eq!(casp_content_category(None, "PetCoatLong"), "Pet Coats");
        assert_eq!(
            casp_content_category(None, "PetAccessoryCollar"),
            "Pet Accessories"
        );
    }

    #[test]
    fn classifies_object_and_build_names() {
        assert_eq!(object_content_category("Modern Sofa"), "Comfort");
        assert_eq!(object_content_category("Wall Lamp"), "Walls");
        assert_eq!(object_content_category("Grand Window"), "Windows");
        assert_eq!(object_content_category("Unknown Thing"), "Other Objects");
    }
}

#[cfg(test)]
mod merge_source_collection_tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn dropped_folder_collects_packages_recursively_and_ignores_other_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("veiga-merger-drop-{nonce}"));
        let nested = root.join("Nested");
        create_dir_all(&nested).unwrap();

        write(root.join("A.package"), b"DBPF").unwrap();
        write(nested.join("B.DBC"), b"DBPF").unwrap();
        write(nested.join("ignore.txt"), b"ignore").unwrap();

        let result = collect_merge_package_files(CollectMergeSourcesPayload {
            paths: vec![root.to_string_lossy().into_owned()],
        })
        .unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|path| path.ends_with("A.package")));
        assert!(result.iter().any(|path| path.ends_with("B.DBC")));
        assert!(!result.iter().any(|path| path.ends_with("ignore.txt")));

        remove_dir_all(root).unwrap();
    }


}
