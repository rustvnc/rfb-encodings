//! Test decoders for round-trip validation of encoders.
//! These are minimal implementations used only for testing.
//!
//! IMPORTANT: These decoders are endian-aware and cross-platform.
//! - Protocol headers (lengths) are always big-endian
//! - Pixel data endianness follows the PixelFormat's big_endian_flag

use flate2::read::ZlibDecoder;
use rfb_encodings::PixelFormat;
use std::io::Read;

/// Calculate bytes per pixel from pixel format
fn bytes_per_pixel(pf: &PixelFormat) -> usize {
    (pf.bits_per_pixel / 8) as usize
}

/// Calculate CPIXEL size according to RFC 6143
fn bytes_per_cpixel(pf: &PixelFormat) -> usize {
    if pf.true_colour_flag != 0 && pf.bits_per_pixel == 32 && pf.depth <= 24 {
        let rgb_in_lower_bytes = (u32::from(pf.red_max) << pf.red_shift) < (1 << 24)
            && (u32::from(pf.green_max) << pf.green_shift) < (1 << 24)
            && (u32::from(pf.blue_max) << pf.blue_shift) < (1 << 24);
        let rgb_in_upper_bytes = pf.red_shift > 7 && pf.green_shift > 7 && pf.blue_shift > 7;

        if rgb_in_lower_bytes || rgb_in_upper_bytes {
            return 3;
        }
    }
    bytes_per_pixel(pf)
}

/// Read a CPIXEL value from bytes according to pixel format endianness
fn read_cpixel(data: &[u8], pf: &PixelFormat) -> u32 {
    let cpixel_size = bytes_per_cpixel(pf);
    match cpixel_size {
        1 => u32::from(data[0]),
        2 => {
            if pf.big_endian_flag != 0 {
                u32::from(u16::from_be_bytes([data[0], data[1]]))
            } else {
                u32::from(u16::from_le_bytes([data[0], data[1]]))
            }
        }
        3 => {
            // 3-byte CPIXEL - determine if 24A or 24B format
            let rgb_in_lower_bytes = (u32::from(pf.red_max) << pf.red_shift) < (1 << 24)
                && (u32::from(pf.green_max) << pf.green_shift) < (1 << 24)
                && (u32::from(pf.blue_max) << pf.blue_shift) < (1 << 24);
            let rgb_in_upper_bytes = pf.red_shift > 7 && pf.green_shift > 7 && pf.blue_shift > 7;
            let big_endian = pf.big_endian_flag != 0;
            let use_24a = (rgb_in_lower_bytes && !big_endian) || (rgb_in_upper_bytes && big_endian);

            if use_24a {
                // 24A: bytes 0, 1, 2
                if big_endian {
                    u32::from(data[0]) << 16 | u32::from(data[1]) << 8 | u32::from(data[2])
                } else {
                    u32::from(data[0]) | u32::from(data[1]) << 8 | u32::from(data[2]) << 16
                }
            } else {
                // 24B: bytes 1, 2, 3 (but we only have 3 bytes, so it's shifted)
                if big_endian {
                    u32::from(data[0]) << 24 | u32::from(data[1]) << 16 | u32::from(data[2]) << 8
                } else {
                    u32::from(data[0]) << 8 | u32::from(data[1]) << 16 | u32::from(data[2]) << 24
                }
            }
        }
        4 => {
            if pf.big_endian_flag != 0 {
                u32::from_be_bytes([data[0], data[1], data[2], data[3]])
            } else {
                u32::from_le_bytes([data[0], data[1], data[2], data[3]])
            }
        }
        _ => panic!("Invalid CPIXEL size"),
    }
}

/// Write a pixel value to bytes according to pixel format
fn write_pixel_to_output(output: &mut [u8], pixel: u32, pf: &PixelFormat) {
    let bpp = bytes_per_pixel(pf);
    match bpp {
        1 => output[0] = pixel as u8,
        2 => {
            let bytes = if pf.big_endian_flag != 0 {
                (pixel as u16).to_be_bytes()
            } else {
                (pixel as u16).to_le_bytes()
            };
            output[0..2].copy_from_slice(&bytes);
        }
        3 => {
            let bytes = if pf.big_endian_flag != 0 {
                let be = pixel.to_be_bytes();
                [be[1], be[2], be[3]]
            } else {
                let le = pixel.to_le_bytes();
                [le[0], le[1], le[2]]
            };
            output[0..3].copy_from_slice(&bytes);
        }
        4 => {
            let bytes = if pf.big_endian_flag != 0 {
                pixel.to_be_bytes()
            } else {
                pixel.to_le_bytes()
            };
            output[0..4].copy_from_slice(&bytes);
        }
        _ => panic!("Invalid bytes per pixel"),
    }
}

/// Decode Raw encoding - just returns the pixels as-is
/// Raw format: just raw pixel data, no header
pub fn decode_raw(encoded: &[u8], _width: u16, _height: u16, _pf: &PixelFormat) -> Vec<u8> {
    encoded.to_vec()
}

/// Decode Zlib encoding
/// Format: 4-byte length (big-endian) + zlib compressed pixel data
pub fn decode_zlib(encoded: &[u8], _pf: &PixelFormat) -> Result<Vec<u8>, String> {
    if encoded.len() < 4 {
        return Err("Zlib data too short".to_string());
    }

    // Length prefix is always big-endian per RFB protocol
    let _len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
    let compressed = &encoded[4..];

    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("Zlib decompression failed: {}", e))?;

    Ok(decompressed)
}

/// Decode ZRLE encoding to raw tile data (decompresses zlib only)
/// Format: 4-byte length (big-endian) + zlib compressed tile data
pub fn decode_zrle_to_tiles(encoded: &[u8]) -> Result<Vec<u8>, String> {
    if encoded.len() < 4 {
        return Err("ZRLE data too short".to_string());
    }

    // Length prefix is always big-endian per RFB protocol
    let len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
    if encoded.len() < 4 + len {
        return Err(format!(
            "ZRLE data truncated: expected {} bytes, got {}",
            len,
            encoded.len() - 4
        ));
    }

    let compressed = &encoded[4..4 + len];

    let mut decoder = ZlibDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| format!("ZRLE zlib decompression failed: {}", e))?;

    Ok(decompressed)
}

/// Fully decode ZRLE to raw pixels
/// This parses the tile structure and reconstructs the original image
pub fn decode_zrle(
    encoded: &[u8],
    width: u16,
    height: u16,
    pf: &PixelFormat,
) -> Result<Vec<u8>, String> {
    let tile_data = decode_zrle_to_tiles(encoded)?;
    let width = width as usize;
    let height = height as usize;
    let cpixel_size = bytes_per_cpixel(pf);
    let output_bpp = bytes_per_pixel(pf);

    // Output buffer - bytes_per_pixel size per pixel
    let mut output = vec![0u8; width * height * output_bpp];

    let mut pos = 0;
    let tile_size = 64;

    for tile_y in (0..height).step_by(tile_size) {
        for tile_x in (0..width).step_by(tile_size) {
            let tile_w = (width - tile_x).min(tile_size);
            let tile_h = (height - tile_y).min(tile_size);

            if pos >= tile_data.len() {
                return Err("ZRLE: unexpected end of tile data".to_string());
            }

            let subencoding = tile_data[pos];
            pos += 1;

            match subencoding {
                0 => {
                    // Raw - copy cpixel data
                    let bytes_needed = tile_w * tile_h * cpixel_size;
                    if pos + bytes_needed > tile_data.len() {
                        return Err("ZRLE: raw tile data truncated".to_string());
                    }
                    for row in 0..tile_h {
                        for col in 0..tile_w {
                            let src_idx = pos + (row * tile_w + col) * cpixel_size;
                            let pixel = read_cpixel(&tile_data[src_idx..], pf);
                            let dst_x = tile_x + col;
                            let dst_y = tile_y + row;
                            let dst_idx = (dst_y * width + dst_x) * output_bpp;
                            write_pixel_to_output(&mut output[dst_idx..], pixel, pf);
                        }
                    }
                    pos += bytes_needed;
                }
                1 => {
                    // Solid color
                    if pos + cpixel_size > tile_data.len() {
                        return Err("ZRLE: solid color data truncated".to_string());
                    }
                    let pixel = read_cpixel(&tile_data[pos..], pf);
                    pos += cpixel_size;

                    for row in 0..tile_h {
                        for col in 0..tile_w {
                            let dst_x = tile_x + col;
                            let dst_y = tile_y + row;
                            let dst_idx = (dst_y * width + dst_x) * output_bpp;
                            write_pixel_to_output(&mut output[dst_idx..], pixel, pf);
                        }
                    }
                }
                2..=16 => {
                    // Packed palette (no RLE)
                    let palette_size = subencoding as usize;
                    if pos + palette_size * cpixel_size > tile_data.len() {
                        return Err("ZRLE: palette data truncated".to_string());
                    }

                    let mut palette = Vec::with_capacity(palette_size);
                    for _ in 0..palette_size {
                        palette.push(read_cpixel(&tile_data[pos..], pf));
                        pos += cpixel_size;
                    }

                    let bits_per_packed = match palette_size {
                        2 => 1,
                        3..=4 => 2,
                        _ => 4,
                    };

                    // Decode packed pixels row by row (each row padded to byte boundary)
                    for row in 0..tile_h {
                        let mut bit_pos = 0;
                        let mut current_byte = 0u8;

                        for col in 0..tile_w {
                            if bit_pos == 0 {
                                if pos >= tile_data.len() {
                                    return Err("ZRLE: packed pixel data truncated".to_string());
                                }
                                current_byte = tile_data[pos];
                                pos += 1;
                                bit_pos = 8;
                            }

                            bit_pos -= bits_per_packed;
                            let idx =
                                ((current_byte >> bit_pos) & ((1 << bits_per_packed) - 1)) as usize;

                            if idx >= palette.len() {
                                return Err(format!("ZRLE: invalid palette index {}", idx));
                            }

                            let dst_x = tile_x + col;
                            let dst_y = tile_y + row;
                            let dst_idx = (dst_y * width + dst_x) * output_bpp;
                            write_pixel_to_output(&mut output[dst_idx..], palette[idx], pf);
                        }
                    }
                }
                128 => {
                    // Plain RLE
                    let mut pixels_remaining = tile_w * tile_h;
                    let mut pixel_idx = 0;

                    while pixels_remaining > 0 {
                        if pos + cpixel_size > tile_data.len() {
                            return Err("ZRLE: RLE color data truncated".to_string());
                        }
                        let pixel = read_cpixel(&tile_data[pos..], pf);
                        pos += cpixel_size;

                        // Read run length
                        let mut run_len = 1usize;
                        loop {
                            if pos >= tile_data.len() {
                                return Err("ZRLE: RLE length data truncated".to_string());
                            }
                            let b = tile_data[pos] as usize;
                            pos += 1;
                            run_len += b;
                            if b != 255 {
                                break;
                            }
                        }

                        // Write run
                        for _ in 0..run_len {
                            if pixels_remaining == 0 {
                                return Err("ZRLE: RLE overflow".to_string());
                            }
                            let row = pixel_idx / tile_w;
                            let col = pixel_idx % tile_w;
                            let dst_x = tile_x + col;
                            let dst_y = tile_y + row;
                            let dst_idx = (dst_y * width + dst_x) * output_bpp;
                            write_pixel_to_output(&mut output[dst_idx..], pixel, pf);
                            pixel_idx += 1;
                            pixels_remaining -= 1;
                        }
                    }
                }
                129..=255 => {
                    // Palette RLE
                    let palette_size = (subencoding - 128) as usize;
                    if pos + palette_size * cpixel_size > tile_data.len() {
                        return Err("ZRLE: palette RLE data truncated".to_string());
                    }

                    let mut palette = Vec::with_capacity(palette_size);
                    for _ in 0..palette_size {
                        palette.push(read_cpixel(&tile_data[pos..], pf));
                        pos += cpixel_size;
                    }

                    let mut pixels_remaining = tile_w * tile_h;
                    let mut pixel_idx = 0;

                    while pixels_remaining > 0 {
                        if pos >= tile_data.len() {
                            return Err("ZRLE: palette RLE index data truncated".to_string());
                        }
                        let index_byte = tile_data[pos];
                        pos += 1;

                        let idx = (index_byte & 0x7F) as usize;
                        if idx >= palette.len() {
                            return Err(format!("ZRLE: invalid palette RLE index {}", idx));
                        }

                        let run_len = if index_byte & 0x80 != 0 {
                            // RLE run
                            let mut len = 1usize;
                            loop {
                                if pos >= tile_data.len() {
                                    return Err("ZRLE: palette RLE length truncated".to_string());
                                }
                                let b = tile_data[pos] as usize;
                                pos += 1;
                                len += b;
                                if b != 255 {
                                    break;
                                }
                            }
                            len
                        } else {
                            1
                        };

                        for _ in 0..run_len {
                            if pixels_remaining == 0 {
                                return Err("ZRLE: palette RLE overflow".to_string());
                            }
                            let row = pixel_idx / tile_w;
                            let col = pixel_idx % tile_w;
                            let dst_x = tile_x + col;
                            let dst_y = tile_y + row;
                            let dst_idx = (dst_y * width + dst_x) * output_bpp;
                            write_pixel_to_output(&mut output[dst_idx..], palette[idx], pf);
                            pixel_idx += 1;
                            pixels_remaining -= 1;
                        }
                    }
                }
                _ => {
                    return Err(format!("ZRLE: unknown subencoding {}", subencoding));
                }
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_zlib_roundtrip() {
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8];

        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Create encoded format: length prefix (big-endian) + compressed data
        let len = compressed.len() as u32;
        let mut encoded = len.to_be_bytes().to_vec();
        encoded.extend_from_slice(&compressed);

        let pf = PixelFormat::rgba32();
        let decoded = decode_zlib(&encoded, &pf).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_bytes_per_cpixel_rgba32() {
        let pf = PixelFormat::rgba32();
        // RGBA32 with depth 24, RGB in lower bytes -> CPIXEL = 3
        assert_eq!(bytes_per_cpixel(&pf), 3);
    }

    #[test]
    fn test_bytes_per_pixel_rgba32() {
        let pf = PixelFormat::rgba32();
        assert_eq!(bytes_per_pixel(&pf), 4);
    }
}
