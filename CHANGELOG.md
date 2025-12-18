# Changelog

All notable changes to rfb-encodings will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.6] - 2025-12-17

### Added

- **Comprehensive Test Suite**: Added 44 automated tests covering all 10 encodings
  - **Golden tests**: Compare encoder output against generated reference files
  - **Round-trip tests**: Encode data, decode with test decoders, verify RGB components match
  - **Smoke tests**: Verify non-deterministic encodings run without error
  - Test decoders for Raw, Zlib, and ZRLE with full CPIXEL and endian support
  - Per-OS golden file generation (`tests/expected/{linux,macos,windows}/`)
  - Test fixture generator binary (`generate_fixture`)

- **ZRLE Bug Fix Tests**: Added specific tests for the buffer overflow bug fix
  - `test_zrle_960x540_original_bug`: Tests the exact dimensions that caused the original panic
  - `test_zrle_non_multiple_of_64_dimensions`: Tests non-aligned dimensions
  - `roundtrip_zrle_full_100x75`: Round-trip test with non-aligned dimensions

- **Feature Flag**: Added `generate-golden` feature for regenerating golden test files

### Changed

- **README**: Updated Testing Status column to reflect comprehensive test coverage
  - All 10 encodings now show test type (Golden, Round-trip, or Smoke)
  - Added new "Testing" section with test commands and coverage details
  - Removed "Untested encodings" note as all encodings are now tested

### Fixed

- **ZRLE Encoding**: Fixed buffer overflow on non-standard dimensions (issue #1)
  - Root cause: Hardcoded `bytes_per_pixel = 4` instead of using `PixelFormat::bytes_per_pixel()`
  - Fixed CPIXEL size calculation per RFC 6143
  - Fixed input buffer validation to use correct bytes per pixel
  - Added `bpp` parameter to `extract_tile` function

## [0.1.5] - 2025-10-27

### Added

- **Documentation**: Comprehensive TurboJPEG setup and licensing information
  - Added TurboJPEG installation instructions for Ubuntu/Debian, macOS, and Windows in README
  - Added "Third-Party Dependencies" section documenting libjpeg-turbo licensing
  - Updated NOTICE file with complete libjpeg-turbo attribution including:
    - BSD-3-Clause license for TurboJPEG API
    - IJG License for libjpeg code
    - zlib License for SIMD extensions
    - Copyright notices for all contributors
  - Added ZYWRLE algorithm acknowledgment to NOTICE file
  - Clarified that libjpeg-turbo is NOT distributed with this crate and users are responsible for license compliance

### Changed

- Updated README to clearly separate installation instructions from licensing information
- Improved License section to reference optional third-party dependencies

## [0.1.3] - 2025-10-23

### Fixed

- **Critical Build Failure**: Fixed compilation errors when using `turbojpeg` feature without `debug-logging`
  - Root cause: Unguarded `log::info!` calls in tight.rs lines 1268 and 1273
  - Error: "failed to resolve: use of unresolved module or unlinked crate `log`"
  - Affected code: TurboJPEG error handling fallback paths
  - **Solution**: Added `#[cfg(feature = "debug-logging")]` guards to log statements
  - Added `#[allow(unused_variables)]` for error variables used only in logging
  - This was a regression in v0.1.2 that broke builds without `debug-logging` feature

## [0.1.2] - 2025-10-23

### Changed

- **Documentation**: Updated README.md with comprehensive encoding testing status table
  - Added table showing all 10 supported encodings with their IDs, descriptions, wire format compliance, and testing status
  - Clearly marked untested encodings (CoRRE, ZlibHex, ZYWRLE) with explanations
  - Added note that untested encodings are RFC 6143 compliant but cannot be tested with noVNC
  - Provided recommendations for using tested alternatives

## [0.1.1] - 2025-10-23

### Fixed

- **macOS CI Build**: Fixed turbojpeg linking errors on macOS in GitHub Actions
  - Added environment variables for library paths in CI workflow
  - Created `build.rs` to automatically configure linker search paths for all platforms
  - macOS: Checks both Apple Silicon (`/opt/homebrew`) and Intel (`/usr/local`) Homebrew paths
  - Windows: Configures vcpkg library paths
  - Linux: Uses standard system library locations

- **Compiler Warnings**: Suppressed unused variable warnings for conditional compilation
  - Fixed `unused_variables` warning for `quality` parameter (only used with turbojpeg/debug-logging features)
  - Fixed `unused_variables` warnings for error variables (only used with debug-logging feature)
  - Added `#[allow(unused_variables)]` attributes for feature-gated code

### Changed

- Improved cross-platform build reliability for turbojpeg feature

## [0.1.0] - 2025-10-23

### Added

**Initial Release** - Complete RFB encoding library

**Encodings Implemented:**
- **Raw** (0) - Uncompressed pixel data
- **RRE** (2) - Rise-and-Run-length Encoding with data loss fix
- **CoRRE** (4) - Compact RRE with 8-bit coordinates
- **Hextile** (5) - 16x16 tile-based encoding
- **Zlib** (6) - Zlib-compressed raw pixels with persistent streams
- **Tight** (7) - Multi-mode compression with all 5 modes:
  - Solid fill (1 color)
  - Mono rect (2 colors, 1-bit bitmap)
  - Indexed palette (3-16 colors)
  - Full-color zlib (lossless)
  - JPEG (lossy, hardware-accelerated with turbojpeg feature)
- **ZlibHex** (8) - Zlib-compressed Hextile with persistent streams
- **ZRLE** (16) - Zlib Run-Length Encoding with persistent streams
- **ZYWRLE** (17) - Wavelet-based lossy compression with persistent streams
- **TightPng** (-260) - PNG-only compression mode

**Pixel Format Support:**
- Full pixel format translation for all color depths
- 8-bit color (RGB332, BGR233, indexed)
- 16-bit color (RGB565, RGB555, BGR565, BGR555)
- 24-bit color (RGB888, BGR888)
- 32-bit color (RGBA32, BGRA32, RGBX, BGRX)
- Big-endian and little-endian support

**Compression Features:**
- Persistent zlib compression streams for optimal performance
- 4 persistent streams for Tight encoding (per RFC 6143)
- Quality level support (0-9 for lossy encodings)
- Compression level support (0-9 for zlib-based encodings)
- `SimpleTightCompressor` for standalone encoding without VNC server context

**Architecture:**
- Pure Rust implementation with memory safety
- No unsafe code in core encoding logic
- `Encoding` trait for pluggable encoders
- `PixelFormat` struct with validation and utilities
- `translate` module for pixel format conversion
- `TightStreamCompressor` trait for persistent compression state

**Features:**
- `turbojpeg` - Optional hardware-accelerated JPEG compression
- `debug-logging` - Optional verbose debug logging

**Documentation:**
- Comprehensive README with usage examples
- Full API documentation with examples
- CONTRIBUTING guidelines
- SECURITY policy
- CODE_OF_CONDUCT

**Testing & CI:**
- Multi-platform CI (Ubuntu, Windows, macOS)
- Rust stable and beta testing
- Clippy linting with zero warnings
- rustfmt formatting checks
- Documentation validation

### Fixed

- **Critical RRE encoding bug**:
  - Root cause: Encoder had "efficiency check" that discarded pixel data
  - Would return 0 subrectangles when encoding was inefficient
  - Caused severe flickering and visual corruption for complex images
  - **Solution**: Always encode all pixels, even if inefficient
  - Ensures correct visual output at all times

### Notes

**Design Philosophy:**
- Reusable across VNC servers, clients, proxies, and recorders
- RFC 6143 compliant for maximum compatibility
- Performance-oriented with zero-copy where possible
- Modular architecture for easy extension

**Tested Encodings:**
- Raw, RRE, CoRRE, Hextile, Zlib, Tight, ZlibHex, ZRLE, TightPng - Fully tested

**Future Considerations:**
- Additional encoding optimizations
- Decoder implementations for VNC clients
- Streaming encoding API
- SIMD optimizations for pixel translation

---

## Release Information

**Initial Release:** v0.1.0 marks the first release of rfb-encodings, providing reusable RFB encoding implementations.

**License:** Apache License 2.0

**Repository:** https://github.com/dustinmcafee/rfb-encodings
