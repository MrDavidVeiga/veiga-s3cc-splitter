//! Read-only thumbnail decoding for The Sims 3 IMG/IMAG/THUM resources.
//! DDS (DXT1/DXT3/DXT5) and RLE-compressed TGA decode to bounded PNG.
//! TS4 RLE2/RLES are separate formats, not valid TS3 DDS headers.

use std::io::Cursor;

const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_PIXELS: u64 = 16_777_216;
const MAX_DIMENSION: u32 = 8192;
const MAX_PREVIEW_EDGE: u32 = 384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewHint {
    Tga,
}

fn select_format(data: &[u8], hint: Option<PreviewHint>) -> Option<image::ImageFormat> {
    if data.starts_with(b"\\x89PNG\\r\\n\\x1a\\n") {
        Some(image::ImageFormat::Png)
    } else if data.starts_with(b"DDS ") {
        Some(image::ImageFormat::Dds)
    } else if hint == Some(PreviewHint::Tga) {
        Some(image::ImageFormat::Tga)
    } else {
        None
    }
}

/// Return a bounded, displayable PNG thumbnail. Reject implausible resource
/// dimensions *before* allocating decoded pixels. Never modify source bytes.
pub fn preview_png(data: &[u8], hint: Option<PreviewHint>, max_edge: u32) -> Option<Vec<u8>> {
    if data.len() < 8 || data.len() > MAX_SOURCE_BYTES {
        return None;
    }
    let format = select_format(data, hint)?;
    let (width, height) = image::io::Reader::with_format(Cursor::new(data), format)
        .into_dimensions()
        .ok()?;
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return None;
    }
    let image = image::load_from_memory_with_format(data, format).ok()?;
    let edge = max_edge.clamp(32, MAX_PREVIEW_EDGE);
    let thumbnail = image.thumbnail(edge, edge);
    let mut encoded = Cursor::new(Vec::new());
    thumbnail.write_to(&mut encoded, image::ImageOutputFormat::Png).ok()?;
    let bytes = encoded.into_inner();
    (bytes.len() <= 2 * 1024 * 1024).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_png_is_decoded_and_scaled_without_mutating_input() {
        let source = image::RgbaImage::from_pixel(512, 256, image::Rgba([10, 30, 50, 200]));
        let mut data = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut data, image::ImageOutputFormat::Png).unwrap();
        let original = data.into_inner();
        let png = preview_png(&original, None, 96).unwrap();
        let result = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
        assert_eq!(result.dimensions(), (96, 48));
        assert_eq!(original[..8], [137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn dxt1_dds_resource_is_decoded_to_png() {
        // Small legacy DDS: one red BC1/DXT1 block of 4x4 pixels.
        let mut dds = vec![0u8; 128 + 8];
        dds[..4].copy_from_slice(b"DDS ");
        dds[4..8].copy_from_slice(&124u32.to_le_bytes());
        dds[12..16].copy_from_slice(&4u32.to_le_bytes());
        dds[16..20].copy_from_slice(&4u32.to_le_bytes());
        dds[76..80].copy_from_slice(&32u32.to_le_bytes());
        dds[80..84].copy_from_slice(&4u32.to_le_bytes());
        dds[84..88].copy_from_slice(b"DXT1");
        dds[128..130].copy_from_slice(&0xf800u16.to_le_bytes());
        dds[130..132].copy_from_slice(&0x07e0u16.to_le_bytes());
        let png = preview_png(&dds, None, 96).unwrap();
        let result = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap().to_rgb8();
        assert_eq!(result.dimensions(), (4, 4));
        assert!(result.get_pixel(0, 0)[0] > 200);
    }

    #[test]
    fn rle_tga_is_supported_only_with_an_explicit_tga_resource_hint() {
        // 2x2 TGA type 10, 24 bpp; one four-pixel red run.
        let mut tga = vec![0u8; 18];
        tga[2] = 10;
        tga[12] = 2; tga[14] = 2; tga[16] = 24;
        tga.extend_from_slice(&[0x83, 0, 0, 255]);
        assert!(preview_png(&tga, None, 96).is_none());
        let png = preview_png(&tga, Some(PreviewHint::Tga), 96).unwrap();
        assert_eq!(image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap().dimensions(), (2, 2));
    }

    #[test]
    fn rejects_oversized_and_unrecognized_texture_data() {
        assert!(preview_png(b"not a texture", None, 96).is_none());
        let data = vec![0u8; MAX_SOURCE_BYTES + 1];
        assert!(preview_png(&data, Some(PreviewHint::Tga), 96).is_none());
    }
}
