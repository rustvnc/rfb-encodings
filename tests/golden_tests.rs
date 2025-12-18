// Golden tests for VNC encodings
// Run normally: cargo test --test golden_tests
// Generate expected outputs: cargo test --test golden_tests --features generate-golden
//
// NOTE: Some encodings (Hextile, RRE, CoRRE) use HashMap which has non-deterministic
// iteration order. These are tested for regression only - output may vary between runs
// but should be consistent within the same build/platform.

use flate2::{Compress, Compression};
use rfb_encodings::zlib::encode_zlib_persistent;
use rfb_encodings::zlibhex::encode_zlibhex_persistent;
use rfb_encodings::zrle::encode_zrle;
use rfb_encodings::zywrle::zywrle_analyze;
use rfb_encodings::{get_encoder, PixelFormat};
use rfb_encodings::{
    ENCODING_CORRE, ENCODING_HEXTILE, ENCODING_RAW, ENCODING_RRE, ENCODING_TIGHT, ENCODING_TIGHTPNG,
};

#[cfg(feature = "generate-golden")]
use std::path::Path;

/// Get the expected output directory for the current OS
fn expected_dir() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "tests/expected/linux"
    }
    #[cfg(target_os = "macos")]
    {
        "tests/expected/macos"
    }
    #[cfg(target_os = "windows")]
    {
        "tests/expected/windows"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "tests/expected/other"
    }
}

/// Compare or generate golden output
fn golden_check(name: &str, data: &[u8]) {
    let path = format!("{}/{}", expected_dir(), name);

    #[cfg(feature = "generate-golden")]
    {
        if let Some(parent) = Path::new(&path).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, data).unwrap();
        println!("Generated: {} ({} bytes)", path, data.len());
    }

    #[cfg(not(feature = "generate-golden"))]
    {
        let expected = std::fs::read(&path).unwrap_or_else(|e| {
            panic!(
                "Failed to read {}: {}. Run with --features generate-golden to create it.",
                path, e
            )
        });
        assert_eq!(data, &expected[..], "Mismatch in {}", name);
    }
}

fn encode_with_trait(encoding_type: i32, data: &[u8], width: u16, height: u16) -> Vec<u8> {
    let encoder = get_encoder(encoding_type).expect("Encoder not found");
    encoder.encode(data, width, height, 85, 6).to_vec()
}

fn load_64x64() -> Vec<u8> {
    std::fs::read("tests/fixtures/frame_64x64.rgba")
        .expect("Run 'cargo run --bin generate_fixture' first")
}

fn load_100x75() -> Vec<u8> {
    std::fs::read("tests/fixtures/frame_100x75.rgba")
        .expect("Run 'cargo run --bin generate_fixture' first")
}

// ============================================================================
// DETERMINISTIC ENCODINGS - these produce identical output across runs
// ============================================================================

// --- Raw encoding (trivial, always deterministic) ---

#[test]
fn golden_raw_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_RAW, &input, 64, 64);
    golden_check("frame_64x64.raw", &encoded);
}

#[test]
fn golden_raw_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_RAW, &input, 100, 75);
    golden_check("frame_100x75.raw", &encoded);
}

// --- ZRLE encoding (zlib may vary by OS, but deterministic per-platform) ---

#[test]
fn golden_zrle_64x64() {
    let input = load_64x64();
    let pf = PixelFormat::rgba32();
    let encoded = encode_zrle(&input, 64, 64, &pf, 6).unwrap();
    golden_check("frame_64x64.zrle", &encoded);
}

#[test]
fn golden_zrle_100x75() {
    let input = load_100x75();
    let pf = PixelFormat::rgba32();
    let encoded = encode_zrle(&input, 100, 75, &pf, 6).unwrap();
    golden_check("frame_100x75.zrle", &encoded);
}

// --- Zlib encoding (zlib may vary by OS) ---

#[test]
fn golden_zlib_64x64() {
    let input = load_64x64();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlib_persistent(&input, &mut compressor).unwrap();
    golden_check("frame_64x64.zlib", &encoded);
}

#[test]
fn golden_zlib_100x75() {
    let input = load_100x75();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlib_persistent(&input, &mut compressor).unwrap();
    golden_check("frame_100x75.zlib", &encoded);
}

// --- Tight encoding (uses zlib internally) ---

#[test]
fn golden_tight_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_TIGHT, &input, 64, 64);
    golden_check("frame_64x64.tight", &encoded);
}

#[test]
fn golden_tight_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_TIGHT, &input, 100, 75);
    golden_check("frame_100x75.tight", &encoded);
}

// --- TightPNG encoding (PNG compression) ---

#[test]
fn golden_tightpng_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_TIGHTPNG, &input, 64, 64);
    golden_check("frame_64x64.tightpng", &encoded);
}

#[test]
fn golden_tightpng_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_TIGHTPNG, &input, 100, 75);
    golden_check("frame_100x75.tightpng", &encoded);
}

// ============================================================================
// NON-DETERMINISTIC ENCODINGS - HashMap iteration order varies
// These tests verify the encoding runs without error.
// Golden comparison is skipped as output varies between runs.
// ============================================================================

#[test]
fn smoke_rre_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_RRE, &input, 64, 64);
    assert!(!encoded.is_empty(), "RRE encoding produced empty output");
}

#[test]
fn smoke_rre_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_RRE, &input, 100, 75);
    assert!(!encoded.is_empty(), "RRE encoding produced empty output");
}

#[test]
fn smoke_corre_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_CORRE, &input, 64, 64);
    assert!(!encoded.is_empty(), "CoRRE encoding produced empty output");
}

#[test]
fn smoke_corre_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_CORRE, &input, 100, 75);
    assert!(!encoded.is_empty(), "CoRRE encoding produced empty output");
}

#[test]
fn smoke_hextile_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_HEXTILE, &input, 64, 64);
    assert!(
        !encoded.is_empty(),
        "Hextile encoding produced empty output"
    );
}

#[test]
fn smoke_hextile_100x75() {
    let input = load_100x75();
    let encoded = encode_with_trait(ENCODING_HEXTILE, &input, 100, 75);
    assert!(
        !encoded.is_empty(),
        "Hextile encoding produced empty output"
    );
}

#[test]
fn smoke_zlibhex_64x64() {
    let input = load_64x64();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlibhex_persistent(&input, 64, 64, &mut compressor).unwrap();
    assert!(
        !encoded.is_empty(),
        "ZlibHex encoding produced empty output"
    );
}

#[test]
fn smoke_zlibhex_100x75() {
    let input = load_100x75();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlibhex_persistent(&input, 100, 75, &mut compressor).unwrap();
    assert!(
        !encoded.is_empty(),
        "ZlibHex encoding produced empty output"
    );
}

/// ZYWRLE is a wavelet transform applied before ZRLE encoding
/// It transforms the pixel data, then ZRLE compresses the result
#[test]
fn smoke_zywrle_64x64() {
    let input = load_64x64();
    let pf = PixelFormat::rgba32();

    // ZYWRLE transform: requires a coefficient buffer
    let mut buf = vec![0i32; 64 * 64];
    let transformed = zywrle_analyze(&input, 64, 64, 1, &mut buf);
    assert!(transformed.is_some(), "ZYWRLE transform failed");

    // Then apply ZRLE encoding to the transformed data
    let transformed_data = transformed.unwrap();
    let encoded = encode_zrle(&transformed_data, 64, 64, &pf, 6).unwrap();
    assert!(
        !encoded.is_empty(),
        "ZYWRLE+ZRLE encoding produced empty output"
    );
}

#[test]
fn smoke_zywrle_100x75() {
    let input = load_100x75();
    let pf = PixelFormat::rgba32();

    // ZYWRLE transform: requires a coefficient buffer
    let mut buf = vec![0i32; 100 * 75];
    let transformed = zywrle_analyze(&input, 100, 75, 1, &mut buf);
    assert!(transformed.is_some(), "ZYWRLE transform failed");

    // Then apply ZRLE encoding to the transformed data
    let transformed_data = transformed.unwrap();
    let encoded = encode_zrle(&transformed_data, 100, 75, &pf, 6).unwrap();
    assert!(
        !encoded.is_empty(),
        "ZYWRLE+ZRLE encoding produced empty output"
    );
}

// ============================================================================
// ROUND-TRIP TESTS - verify encoding is decodable
// These don't require expected files - they verify encode->decode->compare
// ============================================================================

/// Raw encoding round-trip: encoding just reorders bytes, easy to verify
#[test]
fn roundtrip_raw_64x64() {
    let input = load_64x64();
    let encoded = encode_with_trait(ENCODING_RAW, &input, 64, 64);

    // Raw encoding for RGBA32 just outputs the pixels as-is (with potential padding)
    // The output should contain all the pixel data
    assert_eq!(
        encoded.len(),
        input.len(),
        "Raw encoding should preserve size"
    );
}

/// Verify ZRLE output can be decompressed with zlib
#[test]
fn roundtrip_zrle_decompresses_64x64() {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let input = load_64x64();
    let pf = PixelFormat::rgba32();
    let encoded = encode_zrle(&input, 64, 64, &pf, 6).unwrap();

    // ZRLE format: 4-byte length prefix + zlib-compressed data
    assert!(encoded.len() >= 4, "ZRLE output too short");
    let len = u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
    assert_eq!(len, encoded.len() - 4, "ZRLE length prefix mismatch");

    // Verify the zlib data decompresses without error
    let compressed_data = &encoded[4..];
    let mut decoder = ZlibDecoder::new(compressed_data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("ZRLE zlib decompression failed");

    assert!(!decompressed.is_empty(), "ZRLE decompressed to empty data");
}

/// Verify Zlib output can be decompressed (note: pixel format is transformed)
#[test]
fn roundtrip_zlib_decompresses_64x64() {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let input = load_64x64();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlib_persistent(&input, &mut compressor).unwrap();

    // Zlib encoding format: 4-byte length prefix + zlib data
    assert!(encoded.len() >= 4, "Zlib output too short");

    let compressed_data = &encoded[4..];
    let mut decoder = ZlibDecoder::new(compressed_data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("Zlib decompression failed");

    // Zlib encoding transforms pixel format, so we just verify it decompresses
    // to the correct size (same number of pixels, potentially different format)
    assert_eq!(
        decompressed.len(),
        input.len(),
        "Zlib decompressed size mismatch"
    );
}

// ============================================================================
// FULL ROUND-TRIP TESTS - encode -> decode -> compare to original
// Uses the test decoders from tests/decoders.rs
//
// NOTE: The encoders transform pixel data:
// - Raw/Zlib: RGBA -> RGBX (alpha replaced with 0 padding)
// - ZRLE: Uses CPIXEL format (3 bytes for RGBA32 with depth 24)
//
// The tests compare RGB components only, ignoring the alpha/padding byte.
// ============================================================================

mod decoders;

/// Compare two pixel buffers ignoring the alpha/padding byte (4th byte of each pixel)
fn compare_rgb_only(decoded: &[u8], input: &[u8]) -> bool {
    if decoded.len() != input.len() {
        return false;
    }
    for (chunk_d, chunk_i) in decoded.chunks_exact(4).zip(input.chunks_exact(4)) {
        // Compare only RGB (first 3 bytes)
        if chunk_d[0] != chunk_i[0] || chunk_d[1] != chunk_i[1] || chunk_d[2] != chunk_i[2] {
            return false;
        }
    }
    true
}

/// Full round-trip test for Raw encoding
/// Note: Raw encoder converts RGBA to RGBX (alpha -> 0), so we compare RGB only
#[test]
fn roundtrip_raw_full_64x64() {
    let input = load_64x64();
    let pf = PixelFormat::rgba32();
    let encoded = encode_with_trait(ENCODING_RAW, &input, 64, 64);
    let decoded = decoders::decode_raw(&encoded, 64, 64, &pf);
    assert_eq!(decoded.len(), input.len(), "Raw decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "Raw round-trip failed: RGB components don't match"
    );
}

#[test]
fn roundtrip_raw_full_100x75() {
    let input = load_100x75();
    let pf = PixelFormat::rgba32();
    let encoded = encode_with_trait(ENCODING_RAW, &input, 100, 75);
    let decoded = decoders::decode_raw(&encoded, 100, 75, &pf);
    assert_eq!(decoded.len(), input.len(), "Raw decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "Raw round-trip failed: RGB components don't match"
    );
}

/// Full round-trip test for Zlib encoding
/// Note: Zlib encoder also converts RGBA to RGBX, so we compare RGB only
#[test]
fn roundtrip_zlib_full_64x64() {
    let input = load_64x64();
    let pf = PixelFormat::rgba32();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlib_persistent(&input, &mut compressor).unwrap();
    let decoded = decoders::decode_zlib(&encoded, &pf).expect("Zlib decode failed");
    assert_eq!(decoded.len(), input.len(), "Zlib decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "Zlib round-trip failed: RGB components don't match"
    );
}

#[test]
fn roundtrip_zlib_full_100x75() {
    let input = load_100x75();
    let pf = PixelFormat::rgba32();
    let mut compressor = Compress::new(Compression::new(6), true);
    let encoded = encode_zlib_persistent(&input, &mut compressor).unwrap();
    let decoded = decoders::decode_zlib(&encoded, &pf).expect("Zlib decode failed");
    assert_eq!(decoded.len(), input.len(), "Zlib decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "Zlib round-trip failed: RGB components don't match"
    );
}

/// Full round-trip test for ZRLE encoding
/// This is the critical test for the original bug (buffer overflow on Mac)
/// Note: ZRLE uses CPIXEL (3 bytes) for RGBA32 depth 24, decoder reconstructs 4 bytes
#[test]
fn roundtrip_zrle_full_64x64() {
    let input = load_64x64();
    let pf = PixelFormat::rgba32();
    let encoded = encode_zrle(&input, 64, 64, &pf, 6).unwrap();
    let decoded = decoders::decode_zrle(&encoded, 64, 64, &pf).expect("ZRLE decode failed");
    assert_eq!(decoded.len(), input.len(), "ZRLE decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "ZRLE round-trip failed: RGB components don't match"
    );
}

/// Full round-trip test for ZRLE encoding with non-aligned dimensions
/// This specifically tests the fix for the original buffer overflow bug
#[test]
fn roundtrip_zrle_full_100x75() {
    let input = load_100x75();
    let pf = PixelFormat::rgba32();
    let encoded = encode_zrle(&input, 100, 75, &pf, 6).unwrap();
    let decoded = decoders::decode_zrle(&encoded, 100, 75, &pf).expect("ZRLE decode failed");
    assert_eq!(decoded.len(), input.len(), "ZRLE decoded size mismatch");
    assert!(
        compare_rgb_only(&decoded, &input),
        "ZRLE round-trip failed: RGB components don't match"
    );
}

/// Test ZRLE with 16-bit pixel format (no alpha byte issues here)
#[test]
fn roundtrip_zrle_16bpp() {
    let pf = PixelFormat {
        bits_per_pixel: 16,
        depth: 16,
        big_endian_flag: 0, // Little-endian
        true_colour_flag: 1,
        red_max: 31,
        green_max: 63,
        blue_max: 31,
        red_shift: 11,
        green_shift: 5,
        blue_shift: 0,
    };

    // Create a simple 8x8 test pattern with 16bpp
    let mut input = vec![0u8; 8 * 8 * 2];
    for y in 0..8 {
        for x in 0..8 {
            let idx = (y * 8 + x) * 2;
            let r = (x * 4) as u16; // 0-28 fits in 5 bits
            let g = (y * 8) as u16; // 0-56 fits in 6 bits
            let b = ((x + y) * 2) as u16; // 0-28 fits in 5 bits
            let pixel = (r << 11) | (g << 5) | b;
            let bytes = pixel.to_le_bytes();
            input[idx] = bytes[0];
            input[idx + 1] = bytes[1];
        }
    }

    let encoded = encode_zrle(&input, 8, 8, &pf, 6).unwrap();
    let decoded = decoders::decode_zrle(&encoded, 8, 8, &pf).expect("ZRLE decode failed");
    assert_eq!(
        decoded, input,
        "ZRLE 16bpp round-trip failed: decoded doesn't match input"
    );
}
