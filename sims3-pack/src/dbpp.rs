use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const COMPTAIL_0: u32 = 0x0001_0000;
const COMPTAIL_1: u32 = 0x0001_FFFF;

#[derive(Debug, Default, Clone)]
struct DbpfHeader {
    maj_ver: u32,
    min_ver: u32,
    idx_count: u32,
    idx_size: u32,
    idx_ver: u32,
    idx_offs: u32,
}

/// If the file starts with "DBPP", attempt to rewrite the header as a valid "DBPF" package.
/// This is used for "protected" store content and is required by the original tool before reading.
pub fn fix_in_place(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let meta = fs::metadata(path)?;
    if meta.len() < 96 {
        bail!("Corrupted: File header size is smaller than 96 bytes");
    }

    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    f.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic)?;
    if &magic == b"DBPF" {
        return Ok(());
    }
    if &magic != b"DBPP" {
        bail!("Corrupted: File magic does not match DBPF or DBPP");
    }

    let mut hdr = DbpfHeader::default();
    let len = meta.len() as i64;
    if !fix_header(&mut f, &mut hdr, 0, len)? {
        bail!("Corrupted: Could not fix protected file");
    }

    // Rewrite header as DBPF
    f.seek(SeekFrom::Start(0))?;
    f.write_all(b"DBPF")?;
    f.write_u32::<LittleEndian>(hdr.maj_ver)?;
    f.write_u32::<LittleEndian>(hdr.min_ver)?;
    f.write_all(&[0u8; 24])?;
    f.write_u32::<LittleEndian>(hdr.idx_count)?;
    f.write_u32::<LittleEndian>(0)?;
    f.write_u32::<LittleEndian>(hdr.idx_size)?;
    f.write_all(&[0u8; 12])?;
    f.write_u32::<LittleEndian>(hdr.idx_ver)?;
    f.write_u32::<LittleEndian>(hdr.idx_offs)?;
    f.write_all(&[0u8; 28])?;
    Ok(())
}

fn fix_header(
    f: &mut (impl Read + Seek),
    hdr: &mut DbpfHeader,
    start: i64,
    end: i64,
) -> Result<bool> {
    if end - start < 96 {
        return Ok(false);
    }

    f.seek(SeekFrom::Start((end - 4) as u64))?;
    let d32 = f.read_u32::<LittleEndian>()?;
    if d32 != COMPTAIL_0 && d32 != COMPTAIL_1 {
        return Ok(false);
    }

    let mut fc: i32 = 0;
    let mut entry_size: u32;
    let mut idx_count: u32;
    let mut h_size: u32;
    let mut off: i64;
    let mut d32_check: u32;

    loop {
        // search backwards for COMPTAIL marker occurrences
        f.seek(SeekFrom::Start((end - 20) as u64))?;
        off = (end - 20) as i64;
        entry_size = 0;
        idx_count = 1;
        h_size = 0;
        let mut found = 0i32;

        while off - start >= 96 {
            d32_check = f.read_u32::<LittleEndian>()?;
            if d32_check == COMPTAIL_0 || d32_check == COMPTAIL_1 {
                found += 1;
                if found > fc {
                    off = f.stream_position()? as i64;
                    entry_size = (end - off) as u32;
                    break;
                }
            }
            // step back 8 bytes
            let pos = f.stream_position()? as i64;
            f.seek(SeekFrom::Start((pos - 8) as u64))?;
            off = pos - 8;
        }

        if entry_size == 0 {
            fc += 1;
            if fc >= 32 {
                return Ok(false);
            }
            continue;
        }

        // Count index entries by stepping backwards in entry_size blocks until marker stops.
        f.seek(SeekFrom::Start((end - 4) as u64))?;
        off = (end - 4) as i64;
        loop {
            f.seek(SeekFrom::Start((off - entry_size as i64) as u64))?;
            off = f.stream_position()? as i64;
            let v = f.read_u32::<LittleEndian>()?;
            if v == COMPTAIL_0 || v == COMPTAIL_1 {
                idx_count += 1;
            } else {
                break;
            }
        }

        h_size = match entry_size.saturating_sub(16) {
            16 => 4,
            12 => 8,
            8 => 12,
            4 => 16,
            _ => 0xFFFF_FFFF,
        };

        if h_size == 0xFFFF_FFFF {
            d32_check = 0xFFFF_FFFF;
        } else {
            f.seek(SeekFrom::Start(
                (end - (idx_count as i64) * (entry_size as i64) - (h_size as i64)) as u64,
            ))?;
            off = f.stream_position()? as i64;
            d32_check = f.read_u32::<LittleEndian>()?;
        }

        fc += 1;

        if (d32_check & !0x0007) == 0 {
            // Validate entries
            f.seek(SeekFrom::Start((off + h_size as i64) as u64))?;
            let mut i = 0u32;
            while i < idx_count {
                // skip to size fields inside entry
                f.seek(SeekFrom::Current((entry_size as i64) - 12))?;
                let mut cs = f.read_u32::<LittleEndian>()?;
                let ucs = f.read_u32::<LittleEndian>()?;
                let cf = f.read_u16::<LittleEndian>()?;
                let fl = f.read_u16::<LittleEndian>()?;

                if (cs & 0x8000_0000) != 0 {
                    cs &= !0x8000_0000;
                } else {
                    d32_check = 0xFFFF_FFFF;
                    break;
                }

                if fl != 0x0001 {
                    d32_check = 0xFFFF_FFFF;
                    break;
                }

                if cf == 0x0000 {
                    if cs != ucs {
                        d32_check = 0xFFFF_FFFF;
                        break;
                    }
                } else if cs > ucs {
                    d32_check = 0xFFFF_FFFF;
                    break;
                }

                i += 1;
            }

            if d32_check != 0xFFFF_FFFF {
                // success
                hdr.maj_ver = 2;
                hdr.min_ver = 0;
                hdr.idx_count = idx_count;
                hdr.idx_size = (end - off) as u32;
                hdr.idx_ver = 3;
                hdr.idx_offs = (off - start) as u32;
                return Ok(true);
            }
        }

        if (d32_check & !0x0007) == 0 {
            // would have returned above; here means validation failed, continue searching
        }

        if (d32_check & !0x0007) == 0 && fc >= 32 {
            return Ok(false);
        }
        if fc >= 32 {
            return Ok(false);
        }
    }
}
