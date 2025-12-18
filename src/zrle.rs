// Copyright 2025 Dustin McAfee
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! ZRLE (Zlib Run-Length Encoding) implementation for VNC.
//!
//! ZRLE is a highly efficient encoding that combines tiling, palette-based compression,
//! run-length encoding, and zlib compression. It is effective for a wide range of
//! screen content.
//!
//! # Encoding Process
//!
//! 1. The framebuffer region is divided into 64x64 pixel tiles.
//! 2. Each tile is compressed independently.
//! 3. The compressed data for all tiles is concatenated and then compressed as a whole
//!    using zlib.
//!
//! # Tile Sub-encodings
//!
//! Each tile is analyzed and compressed using one of the following methods:
//! - **Raw:** If not otherwise compressible, sent as uncompressed RGBA data.
//! - **Solid Color:** If the tile contains only one color.
//! - **Packed Palette:** If the tile contains 2-16 unique colors. Pixels are sent as
//!   palette indices, which can be run-length encoded.
//! - **Plain RLE:** If the tile has more than 16 colors but is still compressible with RLE.
//!

use bytes::{BufMut, BytesMut};
use flate2::write::ZlibEncoder;
use flate2::{Compress, Compression, FlushCompress};
use std::collections::HashMap;
use std::io::Write;

use crate::{Encoding, PixelFormat};

const TILE_SIZE: usize = 64;

/// Calculates the number of bytes per input pixel based on the pixel format.
/// This is determined by `bits_per_pixel` / 8.
#[inline]
fn bytes_per_pixel(pf: &PixelFormat) -> usize {
    (pf.bits_per_pixel / 8) as usize
}

/// Calculates the CPIXEL size according to RFC 6143.
///
/// CPIXEL is the same as PIXEL except when ALL of these conditions are met:
/// - `true_colour_flag` is non-zero
/// - `bits_per_pixel` is 32
/// - depth is 24 or less
/// - all RGB bits fit in either the least significant 3 bytes or most significant 3 bytes
///
/// When these conditions are met, CPIXEL is 3 bytes. Otherwise it equals `bytes_per_pixel`.
#[inline]
fn bytes_per_cpixel(pf: &PixelFormat) -> usize {
    if pf.true_colour_flag != 0 && pf.bits_per_pixel == 32 && pf.depth <= 24 {
        // Check if RGB fits in least significant 3 bytes (shifts 0-23)
        // fitsInLS3Bytes: (redMax << redShift) < (1<<24) for all colors
        let rgb_in_lower_bytes = (u32::from(pf.red_max) << pf.red_shift) < (1 << 24)
            && (u32::from(pf.green_max) << pf.green_shift) < (1 << 24)
            && (u32::from(pf.blue_max) << pf.blue_shift) < (1 << 24);

        // Check if RGB fits in most significant 3 bytes (shifts > 7)
        // fitsInMS3Bytes: all shifts > 7
        let rgb_in_upper_bytes = pf.red_shift > 7 && pf.green_shift > 7 && pf.blue_shift > 7;

        if rgb_in_lower_bytes || rgb_in_upper_bytes {
            return 3;
        }
    }
    bytes_per_pixel(pf)
}

/// Extracts a pixel value from raw bytes according to the pixel format.
/// Returns a u32 containing the pixel value (for internal processing).
#[inline]
fn read_pixel(data: &[u8], pf: &PixelFormat) -> u32 {
    let bpp = bytes_per_pixel(pf);
    match bpp {
        1 => u32::from(data[0]),
        2 => {
            if pf.big_endian_flag != 0 {
                u32::from(u16::from_be_bytes([data[0], data[1]]))
            } else {
                u32::from(u16::from_le_bytes([data[0], data[1]]))
            }
        }
        4 => {
            if pf.big_endian_flag != 0 {
                u32::from_be_bytes([data[0], data[1], data[2], data[3]])
            } else {
                u32::from_le_bytes([data[0], data[1], data[2], data[3]])
            }
        }
        _ => {
            // Handle 3-byte case (24bpp)
            if pf.big_endian_flag != 0 {
                u32::from(data[0]) << 16 | u32::from(data[1]) << 8 | u32::from(data[2])
            } else {
                u32::from(data[0]) | u32::from(data[1]) << 8 | u32::from(data[2]) << 16
            }
        }
    }
}

/// Determines if we should use 24A format (bytes 0,1,2) or 24B format (bytes 1,2,3)
/// for 3-byte CPIXEL output per RFC 6143.
#[inline]
fn use_cpixel_24a(pf: &PixelFormat) -> bool {
    let rgb_in_lower_bytes = (u32::from(pf.red_max) << pf.red_shift) < (1 << 24)
        && (u32::from(pf.green_max) << pf.green_shift) < (1 << 24)
        && (u32::from(pf.blue_max) << pf.blue_shift) < (1 << 24);
    let rgb_in_upper_bytes = pf.red_shift > 7 && pf.green_shift > 7 && pf.blue_shift > 7;
    let big_endian = pf.big_endian_flag != 0;

    // Use 24A when: (fitsInLS3Bytes && !bigEndian) || (fitsInMS3Bytes && bigEndian)
    (rgb_in_lower_bytes && !big_endian) || (rgb_in_upper_bytes && big_endian)
}

/// Writes a CPIXEL value to the buffer according to the pixel format.
/// For 3-byte CPIXEL (depth <= 24, bpp=32), writes only the significant 3 bytes.
/// Uses 24A format (bytes 0,1,2) or 24B format (bytes 1,2,3) based on pixel layout.
#[inline]
#[allow(clippy::cast_possible_truncation)]
fn write_cpixel(buf: &mut BytesMut, pixel: u32, pf: &PixelFormat) {
    let cpixel_size = bytes_per_cpixel(pf);
    match cpixel_size {
        1 => buf.put_u8(pixel as u8),
        2 => {
            if pf.big_endian_flag != 0 {
                buf.put_u16(pixel as u16);
            } else {
                buf.put_u16_le(pixel as u16);
            }
        }
        3 => {
            // 3-byte CPIXEL: output bytes in client's byte order
            let bytes = if pf.big_endian_flag != 0 {
                pixel.to_be_bytes()
            } else {
                pixel.to_le_bytes()
            };
            if use_cpixel_24a(pf) {
                // 24A: write bytes 0, 1, 2
                buf.put_u8(bytes[0]);
                buf.put_u8(bytes[1]);
                buf.put_u8(bytes[2]);
            } else {
                // 24B: write bytes 1, 2, 3
                buf.put_u8(bytes[1]);
                buf.put_u8(bytes[2]);
                buf.put_u8(bytes[3]);
            }
        }
        4 => {
            if pf.big_endian_flag != 0 {
                buf.put_u32(pixel);
            } else {
                buf.put_u32_le(pixel);
            }
        }
        _ => unreachable!("Invalid CPIXEL size"),
    }
}

/// Analyzes pixel data to count RLE runs, single pixels, and unique colors.
/// Returns: (runs, `single_pixels`, `palette_vec`)
/// CRITICAL: The palette Vec must preserve insertion order (order colors first appear)
/// as required by RFC 6143 for proper ZRLE palette encoding.
/// Optimized: uses inline array for small palettes to avoid `HashMap` allocation.
fn analyze_runs_and_palette(pixels: &[u32]) -> (usize, usize, Vec<u32>) {
    let mut runs = 0;
    let mut single_pixels = 0;
    let mut palette: Vec<u32> = Vec::with_capacity(16); // Most tiles have <= 16 colors

    if pixels.is_empty() {
        return (0, 0, palette);
    }

    let mut i = 0;
    while i < pixels.len() {
        let color = pixels[i];

        // For small palettes (common case), linear search is faster than HashMap
        if palette.len() < 256 && !palette.contains(&color) {
            palette.push(color);
        }

        let mut run_len = 1;
        while i + run_len < pixels.len() && pixels[i + run_len] == color {
            run_len += 1;
        }

        if run_len == 1 {
            single_pixels += 1;
        } else {
            runs += 1;
        }
        i += run_len;
    }
    (runs, single_pixels, palette)
}

/// Encodes a rectangle of pixel data using ZRLE with a persistent compressor.
/// This maintains compression state across rectangles as required by RFC 6143.
///
/// The input data should be in the client's pixel format (as negotiated via `SetPixelFormat`).
/// The encoder uses CPIXEL format for output as specified in RFC 6143.
///
/// # Errors
///
/// Returns an error if zlib compression fails or if the input buffer is too small
#[allow(dead_code)]
#[allow(clippy::cast_possible_truncation)] // ZRLE protocol requires u8/u16/u32 packing of pixel data
pub fn encode_zrle_persistent(
    data: &[u8],
    width: u16,
    height: u16,
    pixel_format: &PixelFormat,
    compressor: &mut Compress,
) -> std::io::Result<Vec<u8>> {
    let width = width as usize;
    let height = height as usize;
    let bpp = bytes_per_pixel(pixel_format);
    let expected_size = width * height * bpp;
    if data.len() < expected_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "ZRLE: input buffer size mismatch: got {} bytes, expected {} bytes for {}x{} image ({} bytes per pixel)",
                data.len(),
                expected_size,
                width,
                height,
                bpp
            ),
        ));
    }
    let mut uncompressed_data = BytesMut::new();

    for y in (0..height).step_by(TILE_SIZE) {
        for x in (0..width).step_by(TILE_SIZE) {
            let tile_w = (width - x).min(TILE_SIZE);
            let tile_h = (height - y).min(TILE_SIZE);

            // Extract tile pixel data
            let tile_data = extract_tile(data, width, x, y, tile_w, tile_h, bpp);

            // Analyze and encode the tile
            encode_tile(
                &mut uncompressed_data,
                &tile_data,
                tile_w,
                tile_h,
                pixel_format,
            );
        }
    }

    // Compress using persistent compressor with Z_SYNC_FLUSH
    // RFC 6143: use persistent zlib stream with dictionary for compression continuity
    let input = &uncompressed_data[..];
    let mut output_buf = vec![0u8; input.len() * 2 + 1024]; // Generous buffer

    let before_out = compressor.total_out();

    // Single compress call with Z_SYNC_FLUSH - this should handle all input
    compressor.compress(input, &mut output_buf, FlushCompress::Sync)?;

    let produced = (compressor.total_out() - before_out) as usize;
    let compressed_output = &output_buf[..produced];

    // Build result with length prefix (big-endian) + compressed data
    let mut result = BytesMut::with_capacity(4 + compressed_output.len());
    result.put_u32(compressed_output.len() as u32);
    result.extend_from_slice(compressed_output);

    #[cfg(feature = "debug-logging")]
    log::info!(
        "ZRLE: compressed {}->{}  bytes ({}x{} tiles)",
        uncompressed_data.len(),
        compressed_output.len(),
        width,
        height
    );

    Ok(result.to_vec())
}

/// Encodes a rectangle of pixel data using the ZRLE encoding.
/// This creates a new compressor for each rectangle (non-RFC compliant, deprecated).
///
/// The input data should be in the client's pixel format (as negotiated via `SetPixelFormat`).
/// The encoder uses CPIXEL format for output as specified in RFC 6143.
///
/// # Errors
///
/// Returns an error if zlib compression fails or if the input buffer is too small
#[allow(clippy::cast_possible_truncation)] // ZRLE protocol requires u8/u16/u32 packing of pixel data
pub fn encode_zrle(
    data: &[u8],
    width: u16,
    height: u16,
    pixel_format: &PixelFormat,
    compression: u8,
) -> std::io::Result<Vec<u8>> {
    let width = width as usize;
    let height = height as usize;
    let bpp = bytes_per_pixel(pixel_format);
    let expected_size = width * height * bpp;
    if data.len() < expected_size {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "ZRLE: input buffer size mismatch: got {} bytes, expected {} bytes for {}x{} image ({} bytes per pixel)",
                data.len(),
                expected_size,
                width,
                height,
                bpp
            ),
        ));
    }

    let compression_level = match compression {
        0 => Compression::fast(),
        1..=3 => Compression::new(u32::from(compression)),
        4..=6 => Compression::default(),
        _ => Compression::best(),
    };
    let mut zlib_encoder = ZlibEncoder::new(Vec::new(), compression_level);
    let mut uncompressed_data = BytesMut::new();

    for y in (0..height).step_by(TILE_SIZE) {
        for x in (0..width).step_by(TILE_SIZE) {
            let tile_w = (width - x).min(TILE_SIZE);
            let tile_h = (height - y).min(TILE_SIZE);

            // Extract tile pixel data
            let tile_data = extract_tile(data, width, x, y, tile_w, tile_h, bpp);

            // Analyze and encode the tile
            encode_tile(
                &mut uncompressed_data,
                &tile_data,
                tile_w,
                tile_h,
                pixel_format,
            );
        }
    }

    zlib_encoder.write_all(&uncompressed_data)?;
    let compressed = zlib_encoder.finish()?;

    // ZRLE requires a 4-byte big-endian length prefix before the zlib data
    let mut result = BytesMut::with_capacity(4 + compressed.len());
    result.put_u32(compressed.len() as u32); // big-endian length
    result.extend_from_slice(&compressed);

    Ok(result.to_vec())
}

/// Encodes a single tile, choosing the best sub-encoding.
/// Handles variable pixel formats according to RFC 6143.
#[allow(clippy::cast_possible_truncation)] // ZRLE palette indices and run lengths limited to u8 per RFC 6143
fn encode_tile(
    buf: &mut BytesMut,
    tile_data: &[u8],
    width: usize,
    height: usize,
    pf: &PixelFormat,
) {
    let cpixel_size = bytes_per_cpixel(pf);
    let bpp = bytes_per_pixel(pf);

    // Quick check for solid color by scanning pixel data directly (avoid allocation)
    if tile_data.len() >= bpp {
        let first_pixel = read_pixel(&tile_data[0..bpp], pf);
        let mut is_solid = true;

        for chunk in tile_data.chunks_exact(bpp).skip(1) {
            if read_pixel(chunk, pf) != first_pixel {
                is_solid = false;
                break;
            }
        }

        if is_solid {
            encode_solid_color_tile(buf, first_pixel, pf);
            return;
        }
    }

    // Convert to u32 pixels for analysis
    let pixels = pixels_to_u32(tile_data, pf);
    let (runs, single_pixels, palette) = analyze_runs_and_palette(&pixels);

    let mut use_rle = false;
    let mut use_palette = false;

    // Start assuming raw encoding size
    let mut estimated_bytes = width * height * cpixel_size;

    let plain_rle_bytes = (cpixel_size + 1) * (runs + single_pixels);

    if plain_rle_bytes < estimated_bytes {
        use_rle = true;
        estimated_bytes = plain_rle_bytes;
    }

    if palette.len() < 128 {
        let palette_size = palette.len();

        // Palette RLE encoding
        let palette_rle_bytes = cpixel_size * palette_size + 2 * runs + single_pixels;

        if palette_rle_bytes < estimated_bytes {
            use_rle = true;
            use_palette = true;
            estimated_bytes = palette_rle_bytes;
        }

        // Packed palette encoding (no RLE)
        if palette_size < 17 {
            let bits_per_packed_pixel = match palette_size {
                2 => 1,
                3..=4 => 2,
                _ => 4, // 5-16 colors
            };
            // Per RFC 6143: each row is padded to byte boundary
            let bytes_per_row = (width * bits_per_packed_pixel).div_ceil(8);
            let packed_bytes = cpixel_size * palette_size + bytes_per_row * height;

            if packed_bytes < estimated_bytes {
                use_rle = false;
                use_palette = true;
                // No need to update estimated_bytes, this is the last check
            }
        }
    }

    if use_palette {
        // Palette (Packed Palette or Packed Palette RLE)
        // Build index lookup from palette (preserves insertion order)
        let color_to_idx: HashMap<_, _> = palette
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, i as u8))
            .collect();

        if use_rle {
            // Packed Palette RLE
            encode_packed_palette_rle_tile(buf, &pixels, &palette, &color_to_idx, pf);
        } else {
            // Packed Palette (no RLE)
            encode_packed_palette_tile(buf, &pixels, width, height, &palette, &color_to_idx, pf);
        }
    } else {
        // Raw or Plain RLE
        if use_rle {
            // Plain RLE - encode directly to buffer (avoid intermediate Vec)
            buf.put_u8(128);
            encode_rle_to_buf(buf, &pixels, pf);
        } else {
            // Raw
            encode_raw_tile(buf, &pixels, pf);
        }
    }
}

/// Extracts a tile from the full framebuffer.
/// Optimized to use a single allocation and bulk copy operations.
///
/// # Arguments
/// * `full_frame` - The complete framebuffer data
/// * `frame_width` - Width of the framebuffer in pixels
/// * `x`, `y` - Top-left corner of the tile in the framebuffer
/// * `width`, `height` - Dimensions of the tile in pixels
/// * `bpp` - Bytes per pixel
#[allow(clippy::uninit_vec)] // Performance optimization: all bytes written via bulk copy before return
fn extract_tile(
    full_frame: &[u8],
    frame_width: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    bpp: usize,
) -> Vec<u8> {
    let tile_size = width * height * bpp;
    let mut tile_data = Vec::with_capacity(tile_size);

    // Use unsafe for performance - we know the capacity is correct
    unsafe {
        tile_data.set_len(tile_size);
    }

    let row_bytes = width * bpp;
    for row in 0..height {
        let src_start = ((y + row) * frame_width + x) * bpp;
        let dst_start = row * row_bytes;
        tile_data[dst_start..dst_start + row_bytes]
            .copy_from_slice(&full_frame[src_start..src_start + row_bytes]);
    }
    tile_data
}

/// Converts pixel data to u32 values for internal processing.
/// Works with any pixel format by using the pixel format's bytes per pixel.
fn pixels_to_u32(data: &[u8], pf: &PixelFormat) -> Vec<u32> {
    let bpp = bytes_per_pixel(pf);
    data.chunks_exact(bpp)
        .map(|chunk| read_pixel(chunk, pf))
        .collect()
}

/// Sub-encoding for a tile with a single color.
fn encode_solid_color_tile(buf: &mut BytesMut, color: u32, pf: &PixelFormat) {
    buf.put_u8(1); // Solid color sub-encoding
    write_cpixel(buf, color, pf);
}

/// Sub-encoding for raw pixel data.
fn encode_raw_tile(buf: &mut BytesMut, pixels: &[u32], pf: &PixelFormat) {
    buf.put_u8(0); // Raw sub-encoding
    for &pixel in pixels {
        write_cpixel(buf, pixel, pf);
    }
}

/// Sub-encoding for a tile with a small palette.
#[allow(clippy::cast_possible_truncation)] // ZRLE palette size limited to 16 colors (u8) per RFC 6143
fn encode_packed_palette_tile(
    buf: &mut BytesMut,
    pixels: &[u32],
    width: usize,
    height: usize,
    palette: &[u32],
    color_to_idx: &HashMap<u32, u8>,
    pf: &PixelFormat,
) {
    let palette_size = palette.len();
    let bits_per_pixel = match palette_size {
        2 => 1,
        3..=4 => 2,
        _ => 4,
    };

    buf.put_u8(palette_size as u8); // Packed palette sub-encoding

    // Write palette as CPIXEL - in insertion order
    for &color in palette {
        write_cpixel(buf, color, pf);
    }

    // Write packed pixel data ROW BY ROW per RFC 6143 ZRLE specification
    // Critical: Each row must be byte-aligned
    for row in 0..height {
        let mut packed_byte = 0;
        let mut nbits = 0;
        let row_start = row * width;
        let row_end = row_start + width;

        for &pixel in &pixels[row_start..row_end] {
            let idx = color_to_idx[&pixel];
            // Pack from MSB: byte = (byte << bppp) | index
            packed_byte = (packed_byte << bits_per_pixel) | idx;
            nbits += bits_per_pixel;

            if nbits >= 8 {
                buf.put_u8(packed_byte);
                packed_byte = 0;
                nbits = 0;
            }
        }

        // Pad remaining bits to MSB at end of row per RFC 6143
        if nbits > 0 {
            packed_byte <<= 8 - nbits;
            buf.put_u8(packed_byte);
        }
    }
}

/// Sub-encoding for a tile with a small palette and RLE.
#[allow(clippy::cast_possible_truncation)] // ZRLE palette size limited to 16 colors (u8) per RFC 6143
fn encode_packed_palette_rle_tile(
    buf: &mut BytesMut,
    pixels: &[u32],
    palette: &[u32],
    color_to_idx: &HashMap<u32, u8>,
    pf: &PixelFormat,
) {
    let palette_size = palette.len();
    buf.put_u8(128 | (palette_size as u8)); // Packed palette RLE sub-encoding

    // Write palette as CPIXEL
    for &color in palette {
        write_cpixel(buf, color, pf);
    }

    // Write RLE data using palette indices per RFC 6143 specification
    let mut i = 0;
    while i < pixels.len() {
        let color = pixels[i];
        let index = color_to_idx[&color];

        let mut run_len = 1;
        while i + run_len < pixels.len() && pixels[i + run_len] == color {
            run_len += 1;
        }

        // Per RFC 6143: length 1 = index only, length 2+ = (index | 128) + length encoding
        if run_len == 1 {
            buf.put_u8(index);
        } else {
            // RLE encoding for runs >= 2 per RFC 6143
            buf.put_u8(index | 128); // Set bit 7 to indicate RLE follows
                                     // Encode run length - 1 using variable-length encoding
            let mut remaining_len = run_len - 1;
            while remaining_len >= 255 {
                buf.put_u8(255);
                remaining_len -= 255;
            }
            buf.put_u8(remaining_len as u8);
        }
        i += run_len;
    }
}

/// Encodes pixel data using run-length encoding directly to buffer (optimized).
#[allow(clippy::cast_possible_truncation)] // ZRLE run lengths encoded as u8 per RFC 6143
fn encode_rle_to_buf(buf: &mut BytesMut, pixels: &[u32], pf: &PixelFormat) {
    let mut i = 0;
    while i < pixels.len() {
        let color = pixels[i];
        let mut run_len = 1;
        while i + run_len < pixels.len() && pixels[i + run_len] == color {
            run_len += 1;
        }
        // Write CPIXEL
        write_cpixel(buf, color, pf);

        // Encode run length - 1 per RFC 6143 ZRLE specification
        // Length encoding: write 255 for each full 255-length chunk, then remainder
        // NO continuation bits - just plain bytes where 255 means "add 255 to length"
        let mut len_to_encode = run_len - 1;
        while len_to_encode >= 255 {
            buf.put_u8(255);
            len_to_encode -= 255;
        }
        buf.put_u8(len_to_encode as u8);

        i += run_len;
    }
}

/// Implements the VNC "ZRLE" (Zlib Run-Length Encoding).
pub struct ZrleEncoding;

impl Encoding for ZrleEncoding {
    fn encode(
        &self,
        data: &[u8],
        width: u16,
        height: u16,
        _quality: u8,
        compression: u8,
    ) -> BytesMut {
        // ZRLE doesn't use quality, but it does use compression.
        let pixel_format = PixelFormat::rgba32(); // Assuming RGBA32 for now
        if let Ok(encoded_data) = encode_zrle(data, width, height, &pixel_format, compression) {
            BytesMut::from(&encoded_data[..])
        } else {
            // Fallback to Raw encoding if ZRLE fails.
            let mut buf = BytesMut::with_capacity(data.len());
            for chunk in data.chunks_exact(4) {
                buf.put_u8(chunk[0]); // R
                buf.put_u8(chunk[1]); // G
                buf.put_u8(chunk[2]); // B
                buf.put_u8(0); // Padding
            }
            buf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

    /// Test that reproduces the GitHub issue #1 buffer overflow.
    /// Dimensions not multiples of 64 caused panic in `extract_tile`.
    #[test]
    fn test_zrle_non_multiple_of_64_dimensions() {
        // 800x600 - not multiples of 64, similar to the reported bug
        let width: u16 = 800;
        let height: u16 = 600;
        let pf = PixelFormat::rgba32();
        let bpp = 4; // 32-bit RGBA

        // Create test framebuffer
        let data = vec![0u8; (width as usize) * (height as usize) * bpp];

        // This would panic before the fix with:
        // "range end index X out of range for slice of length Y"
        let result = encode_zrle(&data, width, height, &pf, 6);
        assert!(
            result.is_ok(),
            "encode_zrle should succeed: {:?}",
            result.err()
        );
    }

    /// Test with the exact dimensions from the original bug report (960x540)
    #[test]
    fn test_zrle_960x540_original_bug() {
        let width: u16 = 960;
        let height: u16 = 540;
        let pf = PixelFormat::rgba32();
        let bpp = 4;

        let data = vec![128u8; (width as usize) * (height as usize) * bpp];

        let result = encode_zrle(&data, width, height, &pf, 6);
        assert!(
            result.is_ok(),
            "encode_zrle should succeed for 960x540: {:?}",
            result.err()
        );
    }

    /// Test buffer size validation - should return error, not panic
    #[test]
    fn test_zrle_buffer_too_small() {
        let width: u16 = 100;
        let height: u16 = 100;
        let pf = PixelFormat::rgba32();

        // Buffer is too small (should be 100*100*4 = 40000 bytes)
        let data = vec![0u8; 1000];

        let result = encode_zrle(&data, width, height, &pf, 6);
        assert!(result.is_err(), "Should return error for undersized buffer");
    }
}
