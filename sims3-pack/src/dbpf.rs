use crate::compression;
use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const MAGIC_DBPF: u32 = 0x4650_4244; // "DBPF" little-endian read => bytes 'D''B''P''F'
const MAJOR: u32 = 2;
const MINOR: u32 = 0;
const INDEX_VERSION: u32 = 3;
const DBPF_HEADER_SIZE: u64 = 96;

#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub type_id: u32,
    pub group: u32,
    pub id1: u32,
    pub id2: u32,
    pub instance: u64,
    pub chunk_offset: u32,
    pub file_size: u32, // masked (no high-bit flag)
    pub mem_size: u32,
    pub compressed: u16,
    pub unknown2: u16,
}

impl ResourceEntry {
    pub fn key_string(&self) -> String {
        format!(
            "0x{:08X}-0x{:08X}-0x{:016X}",
            self.type_id, self.group, self.instance
        )
    }
}

#[derive(Debug)]
pub struct Package {
    pub path: PathBuf,
    pub index_count: u32,
    pub index_length: u32,
    pub index_position: u32,
    pub entries: Vec<ResourceEntry>,
}

impl Package {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
        if meta.len() < DBPF_HEADER_SIZE {
            bail!("Corrupted: File header size is smaller than 96 bytes");
        }

        let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let magic = f.read_u32::<LittleEndian>()?;
        let major = f.read_u32::<LittleEndian>()?;
        let minor = f.read_u32::<LittleEndian>()?;
        // skip 24
        let mut skip = [0u8; 24];
        f.read_exact(&mut skip)?;

        let index_count = f.read_u32::<LittleEndian>()?;
        // skip 4
        f.read_u32::<LittleEndian>()?;
        let index_length = f.read_u32::<LittleEndian>()?;
        // skip 12
        let mut skip12 = [0u8; 12];
        f.read_exact(&mut skip12)?;
        let index_version = f.read_u32::<LittleEndian>()?;
        let index_position = f.read_u32::<LittleEndian>()?;
        // skip 28
        let mut skip28 = [0u8; 28];
        f.read_exact(&mut skip28)?;

        if magic != MAGIC_DBPF {
            bail!("File magic does not match DBPF");
        }
        if major != MAJOR {
            bail!("File major {major} does not match {MAJOR}");
        }
        if minor != MINOR {
            bail!("File minor {minor} does not match {MINOR}");
        }
        if index_version != INDEX_VERSION {
            bail!("File index version {index_version} does not match {INDEX_VERSION}");
        }
        if index_position == 0 {
            bail!("File does not have index position");
        }

        let index_end = (index_position as u64)
            .checked_add(index_length as u64)
            .ok_or_else(|| anyhow::anyhow!("Corrupted: index range overflows u64"))?;
        if (index_position as u64) < DBPF_HEADER_SIZE || index_end > meta.len() {
            bail!(
                "Corrupted: index range {}..{} is outside the file ({} bytes)",
                index_position,
                index_end,
                meta.len()
            );
        }
        if index_length < 4 {
            bail!("Corrupted: index is smaller than its bit flag");
        }

        f.seek(SeekFrom::Start(index_position as u64))?;
        let bit_flag = f.read_u32::<LittleEndian>()?;

        let mut common_count = 0;
        for bit in [1u32, 2, 4, 8] {
            if (bit_flag & bit) != 0 {
                common_count += 1;
            }
        }

        let entry_size = 32u64
            .checked_sub((common_count * 4) as u64)
            .ok_or_else(|| anyhow::anyhow!("Corrupted: invalid common-field mask"))?;
        let expected = (4u64)
            .checked_add((common_count * 4) as u64)
            .and_then(|size| size.checked_add(entry_size.checked_mul(index_count as u64)?))
            .ok_or_else(|| anyhow::anyhow!("Corrupted: index length overflows u64"))?;
        if index_length as u64 != expected {
            bail!(
                "Corrupted: index length {} does not match expected {}",
                index_length,
                expected
            );
        }

        let mut common = vec![0u32; common_count as usize];
        for c in &mut common {
            *c = f.read_u32::<LittleEndian>()?;
        }

        let mut entries = Vec::with_capacity(index_count as usize);
        for _ in 0..index_count {
            let mut ci = 0usize;
            let type_id = if (bit_flag & 1) != 0 {
                common[ci].tap(|_| ci += 1)
            } else {
                f.read_u32::<LittleEndian>()?
            };
            let group = if (bit_flag & 2) != 0 {
                common[ci].tap(|_| ci += 1)
            } else {
                f.read_u32::<LittleEndian>()?
            };
            let id1 = if (bit_flag & 4) != 0 {
                common[ci].tap(|_| ci += 1)
            } else {
                f.read_u32::<LittleEndian>()?
            };
            let id2 = f.read_u32::<LittleEndian>()?;
            let chunk_offset = f.read_u32::<LittleEndian>()?;
            let file_size_raw = f.read_u32::<LittleEndian>()?;
            let mem_size = f.read_u32::<LittleEndian>()?;
            let compressed = f.read_u16::<LittleEndian>()?;
            let unknown2 = f.read_u16::<LittleEndian>()?;

            let file_size = file_size_raw & 0x7FFF_FFFF;
            let instance = ((id1 as u64) << 32) | id2 as u64;

            let data_end = (chunk_offset as u64)
                .checked_add(file_size as u64)
                .ok_or_else(|| anyhow::anyhow!("Corrupted: resource range overflows u64"))?;
            if data_end > meta.len() {
                bail!(
                    "Corrupted: resource {} range {}..{} is outside the file",
                    instance,
                    chunk_offset,
                    data_end
                );
            }

            entries.push(ResourceEntry {
                type_id,
                group,
                id1,
                id2,
                instance,
                chunk_offset,
                file_size,
                mem_size,
                compressed,
                unknown2,
            });
        }

        Ok(Self {
            path: path.to_path_buf(),
            index_count,
            index_length,
            index_position,
            entries,
        })
    }

    pub fn get_raw_data(&self, re: &ResourceEntry) -> Result<Vec<u8>> {
        let file_size = usize::try_from(re.file_size)
            .map_err(|_| anyhow::anyhow!("Resource size does not fit in usize"))?;
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(re.chunk_offset as u64))?;
        let mut buf = vec![0u8; file_size];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn get_uncompressed_data(&self, re: &ResourceEntry) -> Result<Vec<u8>> {
        let raw = self.get_raw_data(re)?;
        if re.file_size != re.mem_size {
            compression::uncompress_stream(
                &raw,
                usize::try_from(re.file_size)
                    .map_err(|_| anyhow::anyhow!("Compressed size does not fit in usize"))?,
                usize::try_from(re.mem_size)
                    .map_err(|_| anyhow::anyhow!("Uncompressed size does not fit in usize"))?,
            )
        } else {
            Ok(raw)
        }
    }

    /// Creates a new empty DBPF package file with a header stub and returns an in-memory representation.
    /// This matches the original tool's behavior (index version 3, index starts at 96).
    pub fn new(path: impl AsRef<Path>, write_extra: bool) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        // Write 96-byte header placeholder.
        f.write_all(b"DBPF")?;
        f.write_u32::<LittleEndian>(MAJOR)?;
        f.write_u32::<LittleEndian>(MINOR)?;
        f.write_all(&[0u8; 24])?;
        f.write_u32::<LittleEndian>(0)?; // indexCount
        f.write_u32::<LittleEndian>(0)?; // unused
        f.write_u32::<LittleEndian>(0)?; // indexLength
        f.write_all(&[0u8; 12])?;
        f.write_u32::<LittleEndian>(INDEX_VERSION)?;
        f.write_u32::<LittleEndian>(96)?; // indexPosition
        f.write_all(&[0u8; 28])?;

        if write_extra {
            // The original wrote a small "extra" region; it's not necessary for core functionality.
            // We keep the flag for parity but do nothing by default.
        }

        Ok(Self {
            path: path.to_path_buf(),
            index_count: 0,
            index_length: 0,
            index_position: 96,
            entries: vec![],
        })
    }

    pub fn write_index_and_header(&mut self) -> Result<()> {
        // Write index at current index_position (caller should have already written resource bytes).
        let mut f = OpenOptions::new().read(true).write(true).open(&self.path)?;
        f.seek(SeekFrom::Start(self.index_position as u64))?;

        // This tool only writes bit_flag=0 (no common fields).
        f.write_u32::<LittleEndian>(0)?;
        for re in &self.entries {
            f.write_u32::<LittleEndian>(re.type_id)?;
            f.write_u32::<LittleEndian>(re.group)?;
            f.write_u32::<LittleEndian>(re.id1)?;
            f.write_u32::<LittleEndian>(re.id2)?;
            f.write_u32::<LittleEndian>(re.chunk_offset)?;
            // Preserve "compressed flag" high bit like the original did when writing.
            f.write_u32::<LittleEndian>(re.file_size | 0x8000_0000)?;
            f.write_u32::<LittleEndian>(re.mem_size)?;
            f.write_u16::<LittleEndian>(re.compressed)?;
            f.write_u16::<LittleEndian>(re.unknown2)?;
        }

        self.index_count = self.entries.len() as u32;
        self.index_length = 4 + self.index_count * 32;

        // Patch header fields.
        f.seek(SeekFrom::Start(36))?;
        f.write_u32::<LittleEndian>(self.index_count)?;
        f.seek(SeekFrom::Start(44))?;
        f.write_u32::<LittleEndian>(self.index_length)?;
        f.seek(SeekFrom::Start(64))?;
        f.write_u32::<LittleEndian>(self.index_position)?;

        Ok(())
    }

    /// Merge multiple packages into one (or multiple) package(s) with a file size limit.
    /// Port of `Package.Merge` from the original tool.
    pub fn merge(
        directory: impl AsRef<Path>,
        base_name: &str,
        sources: &[Package],
        file_size_limit: u64,
    ) -> Result<Vec<PathBuf>> {
        let dir = directory.as_ref();
        fs::create_dir_all(dir)?;

        let mut created = Vec::new();
        let mut merged_count = 1u32;

        let mut out_pkg = Package::new(dir.join(format!("{base_name}.package")), false)?;
        let mut out_f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&out_pkg.path)?;
        out_f.seek(SeekFrom::Start(out_pkg.index_position as u64))?;

        let mut keys: HashSet<String> = HashSet::new();

        for import_pkg in sources {
            for re in &import_pkg.entries {
                // Skip manifest + catalog entries (same as original)
                if re.type_id == 0x73E93EEB || re.type_id == 0x0166038C || re.type_id == 0x7672F0C5
                {
                    continue;
                }
                let k = re.key_string();
                if !keys.insert(k) {
                    continue;
                }

                if out_f.stream_position()? >= file_size_limit {
                    // finalize current package
                    out_pkg.index_position = out_f.stream_position()? as u32;
                    out_pkg.write_index_and_header()?;
                    created.push(out_pkg.path.clone());

                    // start new
                    merged_count += 1;
                    out_pkg = Package::new(
                        dir.join(format!("{base_name}_{merged_count}.package")),
                        false,
                    )?;
                    out_f = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&out_pkg.path)?;
                    out_f.seek(SeekFrom::Start(out_pkg.index_position as u64))?;
                }

                let data = import_pkg
                    .get_raw_data(re)
                    .with_context(|| format!("read resource from {}", import_pkg.path.display()))?;
                let chunk_offset = out_f.stream_position()? as u32;
                out_f.write_all(&data)?;
                out_pkg.entries.push(ResourceEntry {
                    type_id: re.type_id,
                    group: re.group,
                    id1: re.id1,
                    id2: re.id2,
                    instance: re.instance,
                    chunk_offset,
                    file_size: re.file_size,
                    mem_size: re.mem_size,
                    compressed: re.compressed,
                    unknown2: re.unknown2,
                });
            }
        }

        out_pkg.index_position = out_f.stream_position()? as u32;
        out_pkg.write_index_and_header()?;
        created.push(out_pkg.path.clone());

        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::Package;
    use std::fs;
    use std::io::Write;

    fn fixture(index_position: u32, index_length: u32, index_count: u32) -> Vec<u8> {
        let mut bytes = vec![0u8; 96 + 4 + 36];
        bytes[0..4].copy_from_slice(b"DBPF");
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&index_count.to_le_bytes());
        bytes[44..48].copy_from_slice(&index_length.to_le_bytes());
        bytes[60..64].copy_from_slice(&3u32.to_le_bytes());
        bytes[64..68].copy_from_slice(&index_position.to_le_bytes());
        bytes[96..100].copy_from_slice(b"DATA");
        let index = index_position as usize;
        if index + 36 <= bytes.len() {
            bytes[index..index + 4].copy_from_slice(&0u32.to_le_bytes());
            let mut cursor = index + 4;
            for value in [0x1234u32, 0x5678u32, 0x9ABCu32, 0xDEF0u32, 96, 4, 4] {
                bytes[cursor..cursor + 4].copy_from_slice(&value.to_le_bytes());
                cursor += 4;
            }
        }
        bytes
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("s3cc-dbpf-test-{}-{}", std::process::id(), name))
    }

    #[test]
    fn validates_index_and_resource_bounds() {
        let path = temp_path("valid.package");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&fixture(100, 36, 1)).unwrap();
        drop(file);
        let package = Package::load(&path).expect("valid fixture");
        assert_eq!(package.entries.len(), 1);
        assert_eq!(package.get_raw_data(&package.entries[0]).unwrap(), b"DATA");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_index_outside_file() {
        let path = temp_path("invalid.package");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&fixture(200, 36, 1)).unwrap();
        drop(file);
        assert!(Package::load(&path).is_err());
        let _ = fs::remove_file(path);
    }
}

// Small helper: like `.tap()` from other languages.
trait Tap: Sized {
    fn tap<F: FnOnce(&Self)>(self, f: F) -> Self {
        f(&self);
        self
    }
}
impl<T> Tap for T {}
