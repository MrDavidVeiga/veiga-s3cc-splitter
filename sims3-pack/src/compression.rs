use anyhow::{bail, Context, Result};

/// Decompresses Sims 3 DBPF "RefPack" style chunks as used by s3pi.
/// Ported from s3pi's Compression.cs (Peter L Jones, 2009) used in the original tool.
pub fn uncompress_stream(mut input: &[u8], file_size: usize, mem_size: usize) -> Result<Vec<u8>> {
    if input.len() < file_size {
        bail!("input shorter than declared file_size");
    }
    input = &input[..file_size];

    // Header: read 2 bytes, then read a big-endian length field whose size depends on flags.
    if input.len() < 2 {
        bail!("unexpected EOF in compression header");
    }
    let b0 = input[0];
    // let b1 = input[1]; // unused in the original; kept for parity
    let _b1 = input[1];
    input = &input[2..];

    let datalen = if (b0 & 0x80) != 0 { 4 } else { 3 } * if (b0 & 0x01) != 0 { 2 } else { 1 };
    if input.len() < datalen {
        bail!("unexpected EOF reading compressed size field");
    }
    let mut real_size: usize = 0;
    for &b in &input[..datalen] {
        real_size = (real_size << 8) + b as usize;
    }
    input = &input[datalen..];

    if real_size != mem_size {
        bail!("resource indicates uncompressed size 0x{real_size:X} but index says 0x{mem_size:X}");
    }

    // The index can be corrupt and claim an enormous uncompressed size.
    // Grow only for bytes actually decoded; never preallocate that claim.
    let mut out = Vec::new();
    while !input.is_empty() {
        dechunk(&mut input, &mut out, mem_size).context("dechunk failed")?;
    }
    if out.len() != mem_size {
        bail!(
            "decompressed 0x{:X} bytes, expected 0x{:X}",
            out.len(),
            mem_size
        );
    }
    Ok(out)
}

fn dechunk(input: &mut &[u8], out: &mut Vec<u8>, declared_size: usize) -> Result<()> {
    if input.is_empty() {
        bail!("unexpected EOF (no packing byte)");
    }
    let packing = input[0];
    *input = &input[1..];

    let mut copy_size: usize = 0;
    let mut copy_offset: usize = 0;
    let datalen: usize;

    if packing < 0x80 {
        // 0.......; new data 0-3; copy data 3-10; offset 1-1024
        if input.len() < 1 {
            bail!("unexpected EOF in chunk header");
        }
        let b = input[0];
        *input = &input[1..];

        datalen = (packing & 0x03) as usize;
        copy_size = (((packing >> 2) & 0x07) + 3) as usize;
        copy_offset = ((((packing as usize) << 3) & 0x300) | (b as usize)) + 1;
    } else if packing < 0xC0 {
        // 10......; new data 0-3; copy data 4-67; offset 1-16384
        if input.len() < 2 {
            bail!("unexpected EOF in chunk header");
        }
        let b0 = input[0];
        let b1 = input[1];
        *input = &input[2..];

        datalen = ((b0 >> 6) & 0x03) as usize;
        copy_size = ((packing & 0x3F) + 4) as usize;
        copy_offset = ((((b0 as usize) << 8) & 0x3F00) | (b1 as usize)) + 1;
    } else if packing < 0xE0 {
        // 110.....; new data 0-3; copy data 5-1028; offset 1-131072
        if input.len() < 3 {
            bail!("unexpected EOF in chunk header");
        }
        let b0 = input[0];
        let b1 = input[1];
        let b2 = input[2];
        *input = &input[3..];

        datalen = (packing & 0x03) as usize;
        copy_size = ((((packing as usize) << 6) & 0x300) | (b2 as usize)) + 5;
        copy_offset =
            ((((packing as usize) << 12) & 0x10000) | ((b0 as usize) << 8) | (b1 as usize)) + 1;
    } else if packing < 0xFC {
        // 11100000-11111011; uncompressed run: 4..128 bytes
        datalen = (((packing & 0x1F) as usize + 1) << 2) as usize;
    } else {
        // 111111..; uncompressed run: 0..3 bytes
        datalen = (packing & 0x03) as usize;
    }

    // A malformed RefPack command must not write past the decoded size
    // declared by the DBPF index (including back-reference copies).
    let next_size = out.len().checked_add(datalen)
        .and_then(|len| len.checked_add(copy_size))
        .ok_or_else(|| anyhow::anyhow!("Corrupted: RefPack output size overflow"))?;
    if next_size > declared_size {
        bail!("Corrupted: RefPack chunk would decode {} bytes, exceeding declared {} bytes",
            next_size, declared_size);
    }

    if datalen > 0 {
        if input.len() < datalen {
            bail!("unexpected EOF reading literal data");
        }
        out.extend_from_slice(&input[..datalen]);
        *input = &input[datalen..];
    }

    if copy_size > 0 {
        if copy_offset == 0 || copy_offset > out.len() {
            bail!(
                "invalid copy offset {copy_offset} at output pos {}",
                out.len()
            );
        }

        // Copy strategy: if len < offset and offset > 8, copy blocks (CopyA), else bytewise (CopyB)
        if copy_size < copy_offset && copy_offset > 8 {
            copy_a(out, copy_offset, copy_size);
        } else {
            copy_b(out, copy_offset, copy_size);
        }
    }
    Ok(())
}

fn copy_a(out: &mut Vec<u8>, offset: usize, mut len: usize) {
    while len > 0 {
        let chunk = offset.min(len);
        let start = out.len() - offset;
        let src = out[start..start + chunk].to_vec();
        out.extend_from_slice(&src);
        len -= chunk;
    }
}

fn copy_b(out: &mut Vec<u8>, offset: usize, mut len: usize) {
    while len > 0 {
        let b = out[out.len() - offset];
        out.push(b);
        len -= 1;
    }
}

#[cfg(test)]
mod safety_tests {
    use super::uncompress_stream;

    #[test]
    fn decodes_valid_refpack_literal_without_extra_output() {
        let packed = [0x00, 0xFB, 0, 0, 3, 0xFF, b'c', b'a', b't'];
        assert_eq!(uncompress_stream(&packed, packed.len(), 3).unwrap(), b"cat");
    }

    #[test]
    fn rejects_literal_expansion_past_claimed_length() {
        let packed = [0x00, 0xFB, 0, 0, 3, 0xE0, b'a', b'b', b'c', b'd'];
        let error = uncompress_stream(&packed, packed.len(), 3).unwrap_err();
        assert!(error.to_string().contains("dechunk failed"));
        assert!(format!("{error:#}").contains("exceeding declared"));
    }

    #[test]
    fn rejects_backreference_expansion_past_claimed_length() {
        let packed = [0x00, 0xFB, 0, 0, 3, 0xFF, b'c', b'a', b't', 0x00, 0x00];
        assert!(uncompress_stream(&packed, packed.len(), 3).is_err());
    }

    #[test]
    fn rejects_truncated_payload_without_panicking() {
        let packed = [0x00, 0xFB, 0, 0, 3, 0xFF, b'a'];
        assert!(uncompress_stream(&packed, packed.len(), 3).is_err());
    }
}
