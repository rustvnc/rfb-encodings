# rfb-encodings

RFB (Remote Framebuffer) protocol encoding implementations for VNC.

[![Crates.io](https://img.shields.io/crates/v/rfb-encodings.svg)](https://crates.io/crates/rfb-encodings)
[![Documentation](https://docs.rs/rfb-encodings/badge.svg)](https://docs.rs/rfb-encodings)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![Build Status](https://github.com/dustinmcafee/rfb-encodings/workflows/CI/badge.svg)](https://github.com/dustinmcafee/rfb-encodings/actions)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![Downloads](https://img.shields.io/crates/d/rfb-encodings.svg)](https://crates.io/crates/rfb-encodings)

[![LinkedIn](https://img.shields.io/badge/LinkedIn-Dustin%20McAfee-blue?style=flat&logo=linkedin)](https://www.linkedin.com/in/dustinmcafee/)

**Support this project:**

[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-❤-red?style=flat&logo=github-sponsors)](https://github.com/sponsors/dustinmcafee)
[![PayPal](https://img.shields.io/badge/PayPal-Donate-blue?style=flat&logo=paypal)](https://paypal.me/dustinmcafee)
[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-☕-yellow?style=flat&logo=buy-me-a-coffee)](https://buymeacoffee.com/dustinmcafee)
[![Bitcoin](https://img.shields.io/badge/Bitcoin-₿-orange?style=flat&logo=bitcoin)](#crypto-donations)
[![Ethereum](https://img.shields.io/badge/Ethereum-Ξ-blue?style=flat&logo=ethereum)](#crypto-donations)
[![Solana](https://img.shields.io/badge/Solana-◎-purple?style=flat&logo=solana)](#crypto-donations)
[![Monero](https://img.shields.io/badge/Monero-XMR-grey?style=flat&logo=monero)](#crypto-donations)

<details>
<summary id="crypto-donations">💰 Crypto Donations</summary>

**Bitcoin (BTC)**
```
3QVD3H1ryqyxhuf8hNTTuBXSbczNuAKaM8
```

**Ethereum (ETH)**
```
0xaFE28A1Dd57660610Ef46C05EfAA363356e98DC7
```

**Solana (SOL)**
```
6uWx4wuHERBpNxyWjeQKrMLBVte91aBzkHaJb8rhw4rn
```

**Monero (XMR)**
```
8C5aCs7Api3WE67GMw54AhQKnJsCg6CVffCuPxUcaKoiMrnaicyvDch8M2CXTm1DJqhpHKxtLvum9Thw4yHn8zeu7sj8qmC
```

</details>

This crate provides encoding implementations for the VNC/RFB protocol, including all standard encodings defined in RFC 6143. It can be used to build VNC servers, clients, proxies, or recorders.

## Supported Encodings

| Encoding | ID | Description | Wire Format Match | Testing Status |
|----------|----|----|-------------------|----------------|
| **Raw** | 0 | Uncompressed pixels | ✅ 100% | ✅ Tested |
| **RRE** | 2 | Rise-and-Run-length | ✅ 100% | ✅ Tested |
| **CoRRE** | 4 | Compact RRE | ✅ 100% | ⚠️ Untested* |
| **Hextile** | 5 | 16x16 tile-based | ✅ 100% | ✅ Tested |
| **Zlib** | 6 | Zlib-compressed raw | ✅ 100% | ✅ Tested |
| **Tight** | 7 | Multi-mode compression | ✅ 100% (all 5 modes) | ✅ Tested |
| **ZlibHex** | 8 | Zlib-compressed Hextile | ✅ 100% | ⚠️ Untested* |
| **ZRLE** | 16 | Zlib Run-Length | ✅ 100% | ✅ Tested |
| **ZYWRLE** | 17 | Wavelet compression | ✅ 100% | ⚠️ Untested* |
| **TightPng** | -260 | PNG-compressed Tight | ✅ 100% | ✅ Tested |

**\*Untested encodings:** ZlibHex, CoRRE, and ZYWRLE are fully implemented and RFC 6143 compliant but cannot be tested with noVNC (most common VNC client) because noVNC doesn't support them. All three have been code-reviewed and verified against the RFC 6143 specification. Use the widely-supported alternatives: **Zlib** (instead of ZlibHex), **Hextile** (instead of CoRRE), and **ZRLE** (instead of ZYWRLE).

## Features

- **Pure Rust** - Memory-safe implementation with no unsafe code
- **RFC 6143 Compliant** - Follows the official RFB protocol specification
- **Persistent Streams** - Maintains zlib compression state for better compression ratios
- **Pixel Format Translation** - Supports all VNC pixel formats (8/16/24/32-bit)
- **Optional TurboJPEG** - Hardware-accelerated JPEG compression via feature flag

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rfb-encodings = "0.1"
```

### Basic Example

```rust
use rfb_encodings::{Encoding, TightEncoding, PixelFormat};

// Create encoder
let encoder = TightEncoding;

// Encode RGBA pixel data
let rgba_data: Vec<u8> = vec![/* your pixel data */];
let width = 1920;
let height = 1080;
let quality = 9;  // 0-9, higher is better quality
let compression = 6;  // 0-9, higher is more compression

let encoded = encoder.encode(&rgba_data, width, height, quality, compression);
```

### Advanced Usage with Persistent Streams

For better compression with Tight encoding:

```rust
use rfb_encodings::{
    encode_tight_rects, SimpleTightCompressor, PixelFormat
};

let mut compressor = SimpleTightCompressor::new(6);
let client_format = PixelFormat::rgba32();

let rectangles = encode_tight_rects(
    &rgba_data,
    width,
    height,
    9,  // quality
    6,  // compression
    &client_format,
    &mut compressor
);

// Returns Vec<(x, y, width, height, encoded_data)>
for (x, y, w, h, data) in rectangles {
    // Send rectangle to VNC client
}
```

## Features

- `turbojpeg` - Enable TurboJPEG for hardware-accelerated JPEG compression in Tight encoding
- `debug-logging` - Enable verbose debug logging for troubleshooting

Enable features in your `Cargo.toml`:

```toml
[dependencies]
rfb-encodings = { version = "0.1", features = ["turbojpeg", "debug-logging"] }
```

### TurboJPEG Installation

The `turbojpeg` feature requires libjpeg-turbo to be installed on your system:

**Ubuntu/Debian:**
```bash
sudo apt-get install libturbojpeg0-dev
```

**macOS:**
```bash
brew install jpeg-turbo
```

**Windows:**
Download from [libjpeg-turbo.org](https://libjpeg-turbo.org/)

## Architecture

This crate is designed to be reusable across different VNC implementations:

- **`encoding`** modules - Individual encoder implementations
- **`PixelFormat`** - VNC pixel format definition and utilities
- **`translate`** - Pixel format translation between different color depths
- **`TightStreamCompressor`** trait - Interface for persistent zlib streams

## Third-Party Dependencies

### Optional: libjpeg-turbo (TurboJPEG)

When using the `turbojpeg` feature, this crate provides FFI bindings to libjpeg-turbo, which must be installed separately on your system. libjpeg-turbo is licensed under a BSD-style license (BSD-3-Clause, IJG, and zlib components).

**License Information:**
- libjpeg-turbo: [BSD-3-Clause, IJG License, zlib License](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/LICENSE.md)
- **Note:** You are responsible for ensuring compliance with libjpeg-turbo's license terms when using the `turbojpeg` feature.

## License

This crate is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

The `turbojpeg` feature provides optional bindings to libjpeg-turbo, which is licensed separately. See above for details.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
