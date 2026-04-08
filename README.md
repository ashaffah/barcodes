# Barcodes

[![Crates.io](https://img.shields.io/crates/v/barcodes.svg)](https://crates.io/crates/barcodes)
[![Docs.rs](https://docs.rs/barcodes/badge.svg)](https://docs.rs/barcodes)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-edition_2024-orange.svg)]()

A **universal bar/QR code generation library** for Rust, supporting many symbologies.  
Zero external dependencies, `no_std` compatible (requires `alloc`).

## Features

- Zero external dependencies (default)
- `no_std` compatible (requires `alloc`)
- SVG output built-in (`to_svg_string()`)
- Optional image output (PNG, GIF, WebP) via `image` feature
- Supports 15+ barcode symbologies: linear, 2D, and postal

## Installation

Add `barcodes` to your `Cargo.toml`.

**Default (no_std, SVG only):**

```toml
[dependencies]
barcodes = "0.1"
```

**With image output (PNG/GIF/WebP):**

```toml
[dependencies]
barcodes = { version = "0.1", features = ["image"] }
```

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
use barcodes::gs1::databar::GS1DataBar;

let output = GS1DataBar::encode("0950110153001").unwrap();
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

## `no_std` Support

This library is `no_std` compatible by default and only requires the `alloc` crate.  
Enable the `std` feature if you need standard library support.  
Image output (`to_image()`) requires the `image` feature, which implies `std`.

## Modules Overview

| Module              | Description                                      |
| ------------------- | ------------------------------------------------ |
| `barcodes::common`  | Shared traits, types, errors, and output helpers |
| `barcodes::qrcode`  | QR Code Model 2 encoder                          |
| `barcodes::ean_upc` | EAN-13, EAN-8, UPC-A, UPC-E                      |
| `barcodes::linear`  | Code 128, Code 39, ITF                           |
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
