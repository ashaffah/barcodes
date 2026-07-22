# Barcodes

[![Crates.io](https://img.shields.io/crates/v/barcodes.svg)](https://crates.io/crates/barcodes)
[![Docs.rs](https://docs.rs/barcodes/badge.svg)](https://docs.rs/barcodes)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-edition_2024-orange.svg)]()

A **universal bar/QR code generation library** for Rust, supporting many symbologies.  
Zero external dependencies, pure `no_std`, and **zero heap allocation** by default.

## Features

- **Zero heap allocation** by default — encoders write into a caller-provided
  `&mut [bool]` buffer via [`encode_into`](#zero-allocation-core); pure `no_std`,
  no `alloc` required
- Zero external dependencies (default)
- Optional `alloc` feature for owned-output convenience (`encode()` +
  `to_svg_string()`)
- Optional image output (PNG, GIF, WebP) via `image` feature
- Supports 16+ barcode symbologies: linear, 2D, and postal

## Installation

Add `barcodes` to your `Cargo.toml`.

**Default (pure `no_std`, zero allocation):**

```toml
[dependencies]
barcodes = "0.2"
```

**With owned output + SVG string convenience (`alloc`):**

```toml
[dependencies]
barcodes = { version = "0.2", features = ["alloc"] }
```

**With image output (PNG/GIF/WebP — implies `std`):**

```toml
[dependencies]
barcodes = { version = "0.2", features = ["image"] }
```

## Zero-allocation core

Every encoder implements
[`BarcodeEncoder::encode_into`](https://docs.rs/barcodes/latest/barcodes/common/traits/trait.BarcodeEncoder.html),
which writes the symbol's modules into a caller-provided buffer and returns an
`Encoded` describing the written region — no heap, no `alloc`:

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::common::types::Encoded;
use barcodes::ean_upc::ean13::Ean13;

let mut buf = [false; 128]; // stack buffer, one bool per module
let Encoded::Linear { len, height } = Ean13::encode_into("5901234123457", &mut buf).unwrap()
else { unreachable!() };

let bars = &buf[..len]; // true = dark module, false = light
assert_eq!(bars.len(), 95);
let _ = height;
```

2D symbologies return `Encoded::Matrix { width, height }`; their modules fill
`buf[..width * height]` in row-major order.

Render to SVG without allocating via [`common::svg`](https://docs.rs/barcodes/latest/barcodes/common/svg/index.html),
which streams into any `core::fmt::Write` sink.

> The `alloc` feature adds the convenience `Encoder::encode()` (returning an
> owned `BarcodeOutput`) and `.to_svg_string()`. The examples below use it.

> **Upgrading from 0.1.x?** `encode()` / `to_svg_string()` now live behind the
> `alloc` feature. Enable `features = ["alloc"]` to keep the old code unchanged,
> or switch to the zero-allocation `encode_into` shown above. See
> [CHANGELOG.md](CHANGELOG.md#migration-from-01x).

## Supported symbologies

| Symbology                   | Module                       | Status |
| --------------------------- | ---------------------------- | ------ |
| QR Code (Model 2)           | `barcodes::qrcode`           | ✅     |
| EAN-13                      | `barcodes::ean_upc::ean13`   | ✅     |
| EAN-8                       | `barcodes::ean_upc::ean8`    | ✅     |
| UPC-A                       | `barcodes::ean_upc::upca`    | ✅     |
| UPC-E                       | `barcodes::ean_upc::upce`    | ✅     |
| Code 128 (A/B/C)            | `barcodes::linear::code128`  | ✅     |
| Code 39                     | `barcodes::linear::code39`   | ✅     |
| Code 93                     | `barcodes::linear::code93`   | ✅     |
| Codabar (NW-7)              | `barcodes::linear::codabar`  | ✅     |
| ITF (Interleaved 2 of 5)    | `barcodes::linear::itf`      | ✅     |
| GS1-128                     | `barcodes::gs1::gs1_128`     | ✅     |
| GS1 DataBar Omnidirectional | `barcodes::gs1::databar`     | ✅     |
| PDF417                      | `barcodes::twod::pdf417`     | ✅     |
| Data Matrix (ECC 200)       | `barcodes::twod::datamatrix` | ✅     |
| Aztec Code                  | `barcodes::twod::aztec`      | ✅     |
| USPS Intelligent Mail (IMb) | `barcodes::postal::imb`      | ✅     |
| Royal Mail RM4SCC           | `barcodes::postal::rm4scc`   | ✅     |

## Usage

### QR Code

QR implements [`BarcodeEncoder`](#zero-allocation-core) like every other
symbology, so the uniform zero-allocation API works too (defaults: ECC Medium,
automatic version and mask):

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::common::types::Encoded;
use barcodes::qrcode::QrCode;

let mut buf = [false; 177 * 177]; // fits the largest QR (version 40)
let Encoded::Matrix { width, height } =
    QrCode::encode_into("https://example.com", &mut buf).unwrap()
else { unreachable!() };
// buf[y * width + x] == true → dark module
let _ = height;
```

Use `encode_text` directly when you need to control the error-correction level,
version range, or mask:

```rust
use barcodes::qrcode::{QrCode, QrCodeEcc, Version, EncodeTextOptions};

let mut outbuffer = vec![0u8; Version::MAX.buffer_len()];
let mut tempbuffer = vec![0u8; Version::MAX.buffer_len()];

let qr = QrCode::encode_text(
    "Hello, World!",
    &mut tempbuffer,
    &mut outbuffer,
    EncodeTextOptions {
        ecl: QrCodeEcc::Low,
        minversion: Version::MIN,
        maxversion: Version::MAX,
        mask: None,
        boostecl: true,
    },
).unwrap();

println!("Version: {}", qr.version().value());
```

### EAN-13

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::ean13::Ean13;

// 13 digits (check digit validated) or 12 digits (check digit auto-computed)
let output = Ean13::encode("5901234123457").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### UPC-A

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::upca::UpcA;

let output = UpcA::encode("012345678905").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Code 128

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::linear::code128::Code128;

let output = Code128::encode("Hello, World!").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Code 39

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::linear::code39::Code39;

let output = Code39::encode("HELLO WORLD").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Code 93

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::linear::code93::Code93;

let output = Code93::encode("CODE93").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Codabar (NW-7)

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::linear::codabar::Codabar;

// Digits and -$:/.+ ; A/B start/stop guards are added automatically
let output = Codabar::encode("1234567").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### ITF (Interleaved 2 of 5)

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::linear::itf::Itf;

let output = Itf::encode("12345678").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### GS1-128

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::gs1::gs1_128::Gs1_128;

let output = Gs1_128::encode("(01)12345678901231(10)ABC123").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### EAN-8

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::ean8::Ean8;

let output = Ean8::encode("96385074").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### UPC-E

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::upce::UpcE;

let output = UpcE::encode("04252614").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### GS1 DataBar Omnidirectional

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::gs1::databar::DataBar;

let output = DataBar::encode("0950110153001").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### PDF417

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::twod::pdf417::Pdf417;

let output = Pdf417::encode("Hello, PDF417!").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Data Matrix

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::twod::datamatrix::DataMatrix;

let output = DataMatrix::encode("Hello DM").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Aztec Code

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::twod::aztec::Aztec;

let output = Aztec::encode("HELLO AZTEC").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### USPS Intelligent Mail Barcode (IMb)

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::postal::imb::Imb;

let output = Imb::encode("01234567094987654321").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

### Royal Mail RM4SCC

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::postal::rm4scc::Rm4scc;

let output = Rm4scc::encode("EC1A1BB").unwrap();
let svg = output.to_svg_string();
println!("{svg}");
```

## Image Output

Enable the `image` feature to generate raster images (PNG, GIF, WebP).

```rust
// Requires `image` feature enabled
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::ean13::Ean13;

let output = Ean13::encode("5901234123457").unwrap();
let img = output.to_image(2); // module_size = 2px
img.save("ean13.png").unwrap();
```

For QR Code:

```rust
use barcodes::qrcode::{QrCode, QrCodeEcc, Version, EncodeTextOptions};

let mut outbuffer = vec![0u8; Version::MAX.buffer_len()];
let mut tempbuffer = vec![0u8; Version::MAX.buffer_len()];

let qr = QrCode::encode_text(
    "Hello, World!",
    &mut tempbuffer,
    &mut outbuffer,
    EncodeTextOptions {
        ecl: QrCodeEcc::Low,
        minversion: Version::MIN,
        maxversion: Version::MAX,
        mask: None,
        boostecl: true,
    },
).unwrap();

let img = qr.to_image(4); // module_size = 4px
img.save("qrcode.png").unwrap();
```

## `no_std` and features

This library is pure `no_std` by default and performs **no heap allocation** —
the default build does not even link `alloc`, so any accidental allocation is a
compile error.

| Feature     | Adds                                                  | Implies |
| ----------- | ----------------------------------------------------- | ------- |
| _(default)_ | zero-alloc `encode_into` + `core::fmt::Write` SVG     | —       |
| `alloc`     | owned `encode()` → `BarcodeOutput`, `to_svg_string()` | —       |
| `std`       | `std::error::Error` for `EncodeError`                 | `alloc` |
| `image`     | raster output `to_image()` (PNG/GIF/WebP)             | `std`   |

## Modules Overview

| Module              | Description                                      |
| ------------------- | ------------------------------------------------ |
| `barcodes::common`  | Shared traits, types, errors, and output helpers |
| `barcodes::qrcode`  | QR Code Model 2 encoder                          |
| `barcodes::ean_upc` | EAN-13, EAN-8, UPC-A, UPC-E                      |
| `barcodes::linear`  | Code 128, Code 39, Code 93, Codabar, ITF         |
| `barcodes::gs1`     | GS1-128, GS1 DataBar                             |
| `barcodes::twod`    | PDF417, Data Matrix, Aztec Code                  |
| `barcodes::postal`  | USPS IMb, Royal Mail RM4SCC                      |

## Contributing

Contributions are welcome! To get started:

1. Fork the repo
2. Create a feature branch
3. Run `cargo test` and `cargo clippy`
4. Submit a pull request

## License

MIT — see [LICENSE](LICENSE) for details.
