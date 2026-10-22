use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use sims3_pack::dbpf::Package as EasyPackage;
use sims3_pack::{dbpp, ts3pack};

/// Result of resolving an input file into a merged `.package` path.
/// If the input was a `.sims3pack`, the merged package is generated inside a temporary directory
/// that will be deleted automatically when this struct is dropped.
pub struct ResolvedInput {
    pub merged_package: PathBuf,
    pub _temp: Option<TempDir>,
}

pub fn resolve_input(input: impl AsRef<Path>) -> Result<ResolvedInput> {
    let input = input.as_ref();

    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "package" | "dbc" | "ebc" | "sim" | "world" | "lot" => Ok(ResolvedInput {
            merged_package: input.to_path_buf(),
            _temp: None,
        }),
        "sims3pack" => resolve_sims3pack(input),
        _ => bail!("Unsupported input format: {:?}", input),
    }
}

fn resolve_sims3pack(input: &Path) -> Result<ResolvedInput> {
    let tmp = TempDir::new().context("Failed to create temporary directory")?;
    let extract_dir = tmp.path().join("extracted");
    let merged_dir = tmp.path().join("merged");
    fs::create_dir_all(&extract_dir)?;
    fs::create_dir_all(&merged_dir)?;

    // Parse sims3pack and extract packaged payloads
    let s3p = ts3pack::read_sims3pack(input)
        .with_context(|| format!("Failed to parse sims3pack: {input:?}"))?;

    let mut extracted_paths: Vec<PathBuf> = Vec::new();

    for pf in &s3p.packaged_files {
        let name_lc = pf.name.to_ascii_lowercase();
        let is_dbpf_like = name_lc.ends_with(".package")
            || name_lc.ends_with(".dbc")
            || name_lc.ends_with(".ebc")
            || name_lc.ends_with(".sim")
            || name_lc.ends_with(".world")
            || name_lc.ends_with(".lot");

        if !is_dbpf_like {
            continue;
        }

        // Sanitize file name: keep only file name component.
        let file_name = Path::new(&pf.name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("content.package");

        // Avoid collisions: prefix by GUID when available.
        let out_name = if pf.guid.trim().is_empty() {
            file_name.to_string()
        } else {
            format!("{}_{}", pf.guid.replace(['{', '}', '-'], ""), file_name)
        };

        let out_path = extract_dir.join(out_name);

        let bytes = ts3pack::read_packaged_payload(input, &s3p, pf)
            .with_context(|| format!("Failed reading payload {} from sims3pack", pf.name))?;
        fs::write(&out_path, &bytes)
            .with_context(|| format!("Failed writing extracted payload to {out_path:?}"))?;

        // Some sims3packs contain DBPP marker; fix to DBPF if needed.
        // This is safe to call even when no fix is needed.
        let _ = dbpp::fix_in_place(&out_path);

        extracted_paths.push(out_path);
    }

    if extracted_paths.is_empty() {
        bail!(
            "No DBPF-like payloads (.package/.dbc/.ebc/.sim/.world/.lot) found inside sims3pack."
        );
    }

    // Load extracted packages using sims3-pack DBPF reader (it supports compressed entries, etc.)
    let mut sources: Vec<EasyPackage> = Vec::with_capacity(extracted_paths.len());
    for p in &extracted_paths {
        let pkg = EasyPackage::load(p)
            .with_context(|| format!("Failed to read extracted package {p:?}"))?;
        sources.push(pkg);
    }

    // Merge to a single merged package
    // Use a very large limit to avoid splitting (users can still handle the rare case separately).
    let file_size_limit: u64 = 50 * 1024 * 1024 * 1024; // 50 GiB
    let outputs = EasyPackage::merge(&merged_dir, "merged", &sources, file_size_limit)
        .context("Failed merging extracted packages")?;

    let merged_package = outputs
        .get(0)
        .cloned()
        .context("Merge did not produce any output files")?;

    Ok(ResolvedInput {
        merged_package,
        _temp: Some(tmp),
    })
}
