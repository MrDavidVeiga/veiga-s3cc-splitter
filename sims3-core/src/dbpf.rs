pub mod filetypes;

use crate::refpack;
use lazy_init::LazyTransform;
use scroll::{ctx, Pread, LE};
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum IndexType {
    IndexHeader(u8),
    IndexEntry(u8, DBPFIndex),
}

#[derive(PartialEq, Eq, Clone, Copy, Default, Debug)]
struct DBPFIndex {
    resource_type: u32,
    resource_group: u32, // TODO: Flags?
    instance: u64,
    chunk_offset: usize,
    filesize: usize,
    unk1: bool,
    memsize: u32,
    compressed: bool,
    unk2: u16,
}

impl<'a> ctx::TryFromCtx<'a, IndexType> for DBPFIndex {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(src: &'a [u8], ctx: IndexType) -> Result<(Self, Self::Size), Self::Error> {
        let mut data = DBPFIndex::default();
        let mask: u8;
        match ctx {
            IndexType::IndexHeader(m) => mask = m,
            IndexType::IndexEntry(m, template) => {
                mask = !m;
                data = template.clone();
            }
        }

        let offset = &mut 0;
        if (mask & (1 << 0)) != 0 {
            let raw_type = src.gread_with::<u32>(offset, LE)?;
            data.resource_type = raw_type; // would use ResourceType, but there's too many unknowns.
        }
        if (mask & (1 << 1)) != 0 {
            data.resource_group = src.gread_with(offset, LE)?;
        }
        if (mask & (1 << 2)) != 0 {
            // instance_hi
            let field: u32 = src.gread_with(offset, LE)?;
            data.instance = (data.instance & (u32::MAX as u64)) // keep only the low half
                          | ((field as u64) << 32); // replace the high half
        }
        if (mask & (1 << 3)) != 0 {
            // instance_lo
            let field: u32 = src.gread_with(offset, LE)?;
            data.instance = (data.instance & ((u32::MAX as u64) << 32)) // keep only the high half
                          | (field as u64); // replace the low half
        }
        if (mask & (1 << 4)) != 0 {
            data.chunk_offset = src.gread_with::<u32>(offset, LE)? as usize;
        }
        if (mask & (1 << 5)) != 0 {
            let field: u32 = src.gread_with(offset, LE)?;
            data.filesize = (field & 0x7FFFFFFF) as usize;
            data.unk1 = (field & 0x80000000) != 0;
        }
        if (mask & (1 << 6)) != 0 {
            data.memsize = src.gread_with(offset, LE)?;
        }
        if (mask & (1 << 7)) != 0 {
            let field: u32 = src.gread_with(offset, LE)?;
            data.compressed = (field & 0xFFFF) == 0xFFFF;
            data.unk2 = (field >> 16) as u16;
        }

        Ok((data, *offset))
    }
}

#[derive(Debug, PartialEq)]
struct DBPFHeader {
    magic: u32, // "DBPF"
    major: u32,
    minor: u32,
    unk1: Vec<u8>,
    index_entries: u32,
    unk2: Vec<u8>,
    index_size: u32,
    unk3: Vec<u8>,
    index_version: u32, // 3
    index_position: u32,
    unk4: Vec<u8>,
}

impl<'a> ctx::TryFromCtx<'a, scroll::Endian> for DBPFHeader {
    type Error = scroll::Error;
    type Size = usize;

    fn try_from_ctx(
        src: &'a [u8],
        _ctx: scroll::Endian,
    ) -> Result<(Self, Self::Size), Self::Error> {
        let mut offset: usize = 0;
        // read fixed fields
        let magic: u32 = src.gread_with(&mut offset, LE)?;
        let major: u32 = src.gread_with(&mut offset, LE)?;
        let minor: u32 = src.gread_with(&mut offset, LE)?;
        // read unknown byte blocks by slicing
        let unk1 = src[offset..offset + 24].to_vec();
        offset += 24;
        let index_entries: u32 = src.gread_with(&mut offset, LE)?;
        let unk2 = src[offset..offset + 4].to_vec();
        offset += 4;
        let index_size: u32 = src.gread_with(&mut offset, LE)?;
        let unk3 = src[offset..offset + 12].to_vec();
        offset += 12;
        let index_version: u32 = src.gread_with(&mut offset, LE)?;
        let index_position: u32 = src.gread_with(&mut offset, LE)?;
        let unk4 = src[offset..offset + 28].to_vec();
        offset += 28;

        Ok((
            DBPFHeader {
                magic,
                major,
                minor,
                unk1,
                index_entries,
                unk2,
                index_size,
                unk3,
                index_version,
                index_position,
                unk4,
            },
            offset,
        ))
    }
}

pub struct DBPFEntry<'a> {
    pub resource_type: u32,
    pub resource_group: u32, // TODO: Flags?
    pub instance: u64,
    pub unk1: bool,
    pub unk2: u16,
    // This holds a slice to the compressed region before
    // and a possibly owned decompressed slice afterwards
    // TODO: What if I want to use it, discard, and then reuse again later?
    data: LazyTransform<&'a [u8], Cow<'a, [u8]>>,
}

impl<'a> DBPFEntry<'a> {
    pub fn data(&self) -> &[u8] {
        &self
            .data
            .get_or_create(|mem| Cow::Owned(refpack::decompress(mem).unwrap()))
    }
}

pub struct DBPF<'a> {
    pub major: u32,
    pub minor: u32,
    pub files: Vec<DBPFEntry<'a>>,
    by_key: HashMap<(u32, u32, u64), usize>,
    by_instance: HashMap<u64, Vec<usize>>,
    by_type: HashMap<u32, Vec<usize>>,
}

impl<'a> Default for DBPF<'a> {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            files: Vec::new(),
            by_key: HashMap::new(),
            by_instance: HashMap::new(),
            by_type: HashMap::new(),
        }
    }
}

impl<'a> DBPF<'a> {
    pub fn entry_index(&self, resource_type: u32, resource_group: u32, instance: u64) -> Option<usize> {
        self.by_key.get(&(resource_type, resource_group, instance)).copied()
    }

    pub fn entries_for_instance(&self, instance: u64) -> &[usize] {
        self.by_instance.get(&instance).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn entries_for_type(&self, resource_type: u32) -> &[usize] {
        self.by_type.get(&resource_type).map(Vec::as_slice).unwrap_or(&[])
    }
}

impl<'a> ctx::TryFromCtx<'a, ()> for DBPF<'a> {
    type Error = scroll::Error;
    type Size = usize;
    fn try_from_ctx(src: &'a [u8], _ctx: ()) -> Result<(Self, Self::Size), Self::Error> {
        if src.len() < 96 {
            return Err(scroll::Error::Custom("DBPF header is truncated".to_string()));
        }
        let header: DBPFHeader = src.pread(0)?;
        // "DBPF"
        if header.magic != 0x46504244 {
            Err(scroll::Error::Custom(format!(
                "Bad Magic 0x{:x}",
                header.magic
            )))?;
        }

        let files: Vec<_>;
        if header.index_entries != 0 {
            let entry_count = usize::try_from(header.index_entries).map_err(|_| {
                scroll::Error::Custom("DBPF index entry count does not fit usize".to_string())
            })?;
            if entry_count > 10_000_000 {
                return Err(scroll::Error::Custom("DBPF index entry count is unreasonable".to_string()));
            }
            let index_start = usize::try_from(header.index_position).map_err(|_| {
                scroll::Error::Custom("DBPF index position does not fit usize".to_string())
            })?;
            let index_len = usize::try_from(header.index_size).map_err(|_| {
                scroll::Error::Custom("DBPF index size does not fit usize".to_string())
            })?;
            let index_end = index_start.checked_add(index_len).ok_or_else(|| {
                scroll::Error::Custom("DBPF index range overflows usize".to_string())
            })?;
            let index_src = src.get(index_start..index_end).ok_or_else(|| {
                scroll::Error::Custom("DBPF index is outside the mapped file".to_string())
            })?;
            if index_src.len() < 4 {
                return Err(scroll::Error::Custom("DBPF index header is truncated".to_string()));
            }
            let mask: u8 = index_src.pread_with::<u32>(0, LE)? as u8;
            let header_size = mask.count_ones() as usize * 4;
            let entry_size = (!mask).count_ones() as usize * 4;
            let expected_index_len = 4usize
                .checked_add(header_size)
                .and_then(|size| size.checked_add(entry_count.checked_mul(entry_size)?))
                .ok_or_else(|| scroll::Error::Custom("DBPF index size overflows usize".to_string()))?;
            if expected_index_len != index_len {
                return Err(scroll::Error::Custom(format!(
                    "DBPF index size {} does not match expected {}",
                    index_len, expected_index_len
                )));
            }
            let mut index = vec![DBPFIndex::default(); entry_count];
            let offset = &mut 4;
            let index_header = index_src.gread_with(offset, IndexType::IndexHeader(mask))?;
            index_src.gread_inout_with(
                offset,
                &mut index,
                IndexType::IndexEntry(mask, index_header),
            )?;
            files = index
                .into_iter()
                .map(|index| {
                    let end = index.chunk_offset.checked_add(index.filesize).ok_or_else(|| {
                        refpack::decompress::Error::ExpectedAnotherByte
                    })?;
                    let raw = src.get(index.chunk_offset..end).ok_or_else(|| {
                        refpack::decompress::Error::ExpectedAnotherByte
                    })?;
                    let data = LazyTransform::new(raw);
                    if !index.compressed {
                        // We don't need to defer this.
                        data.get_or_create(|mem| Cow::Borrowed(mem));
                    }

                    Ok(DBPFEntry {
                        resource_type: index.resource_type,
                        resource_group: index.resource_group,
                        instance: index.instance,
                        unk1: index.unk1,
                        unk2: index.unk2,
                        data,
                    })
                })
                .collect::<Result<_, _>>()
                .map_err(|err: refpack::decompress::Error| {
                    scroll::Error::Custom(err.to_string())
                })?;
        } else {
            files = Vec::with_capacity(0);
        }

        let mut data = DBPF {
            major: header.major,
            minor: header.minor,
            files: files, // already checked for error.
            ..DBPF::default()
        };
        data.by_key.reserve(data.files.len());
        for (position, entry) in data.files.iter().enumerate() {
            data.by_key
                .entry((entry.resource_type, entry.resource_group, entry.instance))
                .or_insert(position);
            data.by_instance.entry(entry.instance).or_default().push(position);
            data.by_type.entry(entry.resource_type).or_default().push(position);
        }
        Ok((data, src.len()))
    }
}

impl<'a> DBPF<'a> {
    pub fn new(mem: &'a [u8]) -> Result<DBPF<'a>, scroll::Error> {
        mem.pread::<DBPF>(0)
    }
}

#[cfg(test)]
mod tests {
    use super::DBPF;

    fn fixture(chunk_offset: u32, file_size: u32) -> Vec<u8> {
        let index_position = 96usize;
        let index_size = 36usize;
        let mut bytes = vec![0u8; index_position + index_size + file_size as usize];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
        bytes[44..48].copy_from_slice(&(index_size as u32).to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        bytes[64..68].copy_from_slice(&(index_position as u32).to_le_bytes());
        let index = index_position;
        bytes[index..index + 4].copy_from_slice(&0u32.to_le_bytes());
        let mut cursor = index + 4;
        for value in [0x1234u32, 0x5678u32, 0x9ABCu32, 0xDEF0u32, chunk_offset, file_size, file_size] {
            bytes[cursor..cursor + 4].copy_from_slice(&value.to_le_bytes());
            cursor += 4;
        }
        bytes[cursor..cursor + 2].copy_from_slice(&0u16.to_le_bytes());
        bytes[cursor + 2..cursor + 4].copy_from_slice(&0u16.to_le_bytes());
        bytes
    }

    #[test]
    fn rejects_truncated_header_without_panicking() {
        assert!(DBPF::new(&[0u8; 95]).is_err());
    }

    #[test]
    fn rejects_resource_outside_mapped_file() {
        let bytes = fixture(136, 8);
        assert!(DBPF::new(&bytes).is_err());
    }

    #[test]
    fn builds_constant_time_resource_indexes() {
        let bytes = fixture(132, 4);
        let package = DBPF::new(&bytes).expect("valid fixture");
        assert_eq!(package.entry_index(0x1234, 0x5678, 0x9ABC_0000_DEF0), Some(0));
        assert_eq!(package.entries_for_instance(0x9ABC_0000_DEF0), &[0]);
        assert_eq!(package.entries_for_type(0x1234), &[0]);
    }
}
