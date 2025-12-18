//! Test Fixture Generator
//!
//! Generates deterministic RGBA test images used as inputs for golden tests.
//! These fixtures are identical on every platform (no randomness), while the
//! encoded outputs may vary per-OS due to zlib implementation differences.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin generate_fixture
//! ```
//!
//! # Generated Files
//!
//! - `tests/fixtures/frame_64x64.rgba` (16,384 bytes)
//!   - 64x64 image with 4 quadrants for testing different encoding scenarios:
//!     - Top-left: red horizontal gradient (tests gradient compression)
//!     - Top-right: green vertical gradient (tests gradient compression)
//!     - Bottom-left: solid blue (tests solid fill optimization)
//!     - Bottom-right: checkerboard (tests pattern encoding)
//!
//! - `tests/fixtures/frame_100x75.rgba` (30,000 bytes)
//!   - 100x75 image with red/green gradients
//!   - Non-64-aligned dimensions specifically test the ZRLE buffer overflow
//!     bug fix (issue #1) where non-standard dimensions caused panics

fn main() {
    // Generate a 64x64 test pattern with gradients and solid areas
    let mut pixels = Vec::with_capacity(64 * 64 * 4);

    for y in 0..64 {
        for x in 0..64 {
            // Create quadrants with different patterns
            let (r, g, b, a) = if x < 32 && y < 32 {
                // Top-left: horizontal gradient
                ((x * 8) as u8, 0, 0, 255)
            } else if x >= 32 && y < 32 {
                // Top-right: vertical gradient
                (0, (y * 8) as u8, 0, 255)
            } else if x < 32 && y >= 32 {
                // Bottom-left: solid color
                (0, 0, 200, 255)
            } else {
                // Bottom-right: checkerboard
                if (x + y) % 2 == 0 {
                    (255, 255, 255, 255)
                } else {
                    (0, 0, 0, 255)
                }
            };
            pixels.extend_from_slice(&[r, g, b, a]);
        }
    }

    std::fs::write("tests/fixtures/frame_64x64.rgba", &pixels).unwrap();
    println!(
        "Generated tests/fixtures/frame_64x64.rgba ({} bytes)",
        pixels.len()
    );

    // Also generate a non-64-aligned image to test edge cases (the original bug)
    let mut pixels_100x75 = Vec::with_capacity(100 * 75 * 4);
    for y in 0..75 {
        for x in 0..100 {
            let r = ((x * 255) / 100) as u8;
            let g = ((y * 255) / 75) as u8;
            let b = 128;
            pixels_100x75.extend_from_slice(&[r, g, b, 255]);
        }
    }
    std::fs::write("tests/fixtures/frame_100x75.rgba", &pixels_100x75).unwrap();
    println!(
        "Generated tests/fixtures/frame_100x75.rgba ({} bytes)",
        pixels_100x75.len()
    );
}
