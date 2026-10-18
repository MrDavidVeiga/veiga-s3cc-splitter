use super::ResourceType;
use crate::dbpf::DBPF;
use num_traits::ToPrimitive;
use scroll::{ctx, Pread};
use std::collections::BTreeMap;

struct NameMapEntry {
    pub instance: u64,
    pub name: String,
}

impl<'a> ctx::TryFromCtx<'a, ()> for NameMapEntry {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(src: &'a [u8], _ctx: ()) -> Result<(Self, Self::Size), Self::Error> {
        let instance: u64 = src.pread(0)?;
        let length = src.pread::<u32>(8)? as usize;
        let name: &str = src.pread_with(12, ctx::StrCtx::Length(length))?;
        Ok((
            NameMapEntry {
                instance,
                name: name.to_string(),
            },
            12 + length,
        ))
    }
}

const MAX_NMAP_ENTRIES: usize = 1_000_000;

fn gather_names_from_data(
    data: &[u8],
    name_map: &mut BTreeMap<u64, String>,
) {
    let mut offset = 4usize;
    if data.len() < offset.saturating_add(4) {
        return;
    }
    let Ok(count) = data.gread::<u32>(&mut offset).map(|value| value as usize) else {
        return;
    };

    // The old implementation used Vec::set_len on uninitialized String
    // values. A malformed or simply large NMAP could therefore corrupt the
    // allocator before the requested package was displayed. Parse one entry
    // at a time and cap the count by both the format's minimum entry size and
    // a conservative sanity limit.
    let maximum_from_bytes = data.len().saturating_sub(offset) / 12;
    let count = count.min(maximum_from_bytes).min(MAX_NMAP_ENTRIES);
    for _ in 0..count {
        let previous_offset = offset;
        let Ok(entry) = data.gread_with::<NameMapEntry>(&mut offset, ()) else {
            break;
        };
        if offset == previous_offset {
            break;
        }
        if let Some(old) = name_map.insert(entry.instance, entry.name) {
            println!("Name Table Conflict for instance {:#08X}", entry.instance);
            println!("    Old: {}", old);
        }
    }
}

// TODO: This should move into dbpf::filetypes::nmap
pub fn gather_names_into(
    package: &DBPF<'_>,
    name_map: &mut BTreeMap<u64, String>,
) -> Result<(), scroll::Error> {
    for entry in package
        .files
        .iter()
        .filter(|entry| entry.resource_type == ResourceType::NMAP.to_u32().unwrap())
    {
        gather_names_from_data(entry.data(), name_map);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::gather_names_from_data;
    use std::collections::BTreeMap;

    #[test]
    fn malformed_count_does_not_allocate_uninitialized_strings() {
        let mut data = vec![0u8; 8];
        data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut names = BTreeMap::new();
        gather_names_from_data(&data, &mut names);
        assert!(names.is_empty());
    }

    #[test]
    fn valid_name_entries_are_read_incrementally() {
        let mut data = vec![0u8; 4 + 4 + 8 + 4 + 3];
        data[4..8].copy_from_slice(&1u32.to_le_bytes());
        data[8..16].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
        data[16..20].copy_from_slice(&3u32.to_le_bytes());
        data[20..23].copy_from_slice(b"abc");
        let mut names = BTreeMap::new();
        gather_names_from_data(&data, &mut names);
        assert_eq!(names.get(&0x1122_3344_5566_7788), Some(&"abc".to_string()));
    }
}
