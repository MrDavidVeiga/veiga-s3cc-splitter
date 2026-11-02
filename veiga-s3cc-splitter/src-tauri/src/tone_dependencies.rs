//! Structural TONE dependency reader for The Sims 3 (resource 0x0354796A).
//! The TGI block is anchored by a relative offset, NOT by a TONE's own instance
//! or similarity between resource names. Unknown layouts fail closed.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToneReference {
    pub resource_type: u32,
    pub resource_group: u32,
    pub instance: u64,
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes: [u8; 4] = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| "TONE truncated while reading a DWORD".to_string())?
        .try_into()
        .map_err(|_| "TONE has an invalid DWORD".to_string())?;
    Ok(u32::from_le_bytes(bytes))
}

/// Reads every non-null TGI from a format 4 or format 6 TONE.
///
/// The TGI offset in bytes 4..8 is relative to the position immediately after
/// that DWORD (byte 8). The size in bytes 8..12 includes the count DWORD.
/// References absent from the current DBPF may legitimately live in EA game
/// packages or another installed mod, so callers must not mark them as corrupt
/// solely because they are external.
pub(crate) fn parse_tone_references(data: &[u8]) -> Result<Vec<ToneReference>, String> {
    if data.len() < 12 {
        return Err("TONE header is truncated".to_string());
    }
    let version = read_u32(data, 0)?;
    if !matches!(version, 4 | 6) {
        return Err(format!(
            "Unsupported TONE format version {version}; cannot safely export its dependencies"
        ));
    }

    let offset = read_u32(data, 4)? as usize;
    let block_size = read_u32(data, 8)? as usize;
    let block_start = 8usize
        .checked_add(offset)
        .ok_or_else(|| "TONE TGI offset overflow".to_string())?;
    let block_end = block_start
        .checked_add(block_size)
        .ok_or_else(|| "TONE TGI size overflow".to_string())?;
    if block_start < 12 || block_size < 4 || block_end > data.len() {
        return Err("TONE TGI block is out of bounds".to_string());
    }

    let count = read_u32(data, block_start)? as usize;
    if count > 4096 {
        return Err("TONE contains an implausible TGI reference count".to_string());
    }
    let expected_size = count
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(4))
        .ok_or_else(|| "TONE TGI reference count overflow".to_string())?;
    if expected_size != block_size {
        return Err(format!(
            "TONE TGI block size mismatch (declared {block_size}, expected {expected_size})"
        ));
    }

    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let pos = block_start + 4 + index * 16;
        let resource_type = read_u32(data, pos)?;
        let resource_group = read_u32(data, pos + 4)?;
        let instance = u64::from_le_bytes(
            data[pos + 8..pos + 16]
                .try_into()
                .map_err(|_| "TONE TGI instance is truncated".to_string())?,
        );
        if resource_type == 0 && resource_group == 0 && instance == 0 {
            continue; // Null placeholder, not a real dependency.
        }
        result.push(ToneReference {
            resource_type,
            resource_group,
            instance,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{parse_tone_references, ToneReference};

    fn example(version: u32) -> Vec<u8> {
        let refs = [
            ToneReference { resource_type: 0x00B2D882, resource_group: 0, instance: 0x1122334455667788 },
            ToneReference { resource_type: 0, resource_group: 0, instance: 0 },
            ToneReference { resource_type: 0x00B2D882, resource_group: 0x01020304, instance: 0xFFEEDDCCBBAA9988 },
        ];
        let mut data = Vec::new();
        data.extend_from_slice(&version.to_le_bytes());
        data.extend_from_slice(&16u32.to_le_bytes()); // Block starts at 8 + 16 = 24.
        data.extend_from_slice(&(4u32 + refs.len() as u32 * 16).to_le_bytes());
        data.resize(24, 0xAA);
        data.extend_from_slice(&(refs.len() as u32).to_le_bytes());
        for r in refs {
            data.extend_from_slice(&r.resource_type.to_le_bytes());
            data.extend_from_slice(&r.resource_group.to_le_bytes());
            data.extend_from_slice(&r.instance.to_le_bytes());
        }
        data
    }

    #[test]
    fn format_4_uses_real_resource_tgis_not_tone_instance_or_names() {
        let refs = parse_tone_references(&example(4)).unwrap();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].instance, 0x1122334455667788);
        assert_eq!(refs[1].resource_group, 0x01020304);
        assert_eq!(refs[1].instance, 0xFFEEDDCCBBAA9988);
    }

    #[test]
    fn format_6_reads_same_tgi_block_layout() {
        assert_eq!(parse_tone_references(&example(6)).unwrap().len(), 2);
    }

    #[test]
    fn refuses_truncated_and_inconsistent_tgi_tables() {
        let mut truncated = example(6);
        truncated.truncate(truncated.len() - 1);
        assert!(parse_tone_references(&truncated).is_err());

        let mut mismatch = example(4);
        mismatch[8..12].copy_from_slice(&20u32.to_le_bytes());
        assert!(parse_tone_references(&mismatch).is_err());
    }

    #[test]
    fn refuses_unknown_tone_formats_instead_of_silently_losing_textures() {
        assert!(parse_tone_references(&example(7)).is_err());
        assert!(parse_tone_references(&[]).is_err());
    }
}
