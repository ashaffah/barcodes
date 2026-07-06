# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/) (with `0.x` minor bumps signalling
breaking changes).

## [0.2.0] — Unreleased

### Breaking

- **Zero-allocation core.** The primary API is now
  [`BarcodeEncoder::encode_into(input, &mut [bool])`](https://docs.rs/barcodes/latest/barcodes/common/traits/trait.BarcodeEncoder.html),
  which writes a symbol's modules into a caller-provided buffer and returns an
  `Encoded { Linear | Matrix }` describing the written region. The crate is now
  pure `no_std` with **no heap allocation** by default.
- The owned-output convenience methods `encode()` (returning `BarcodeOutput`)
  and `to_svg_string()` moved behind the new **`alloc`** feature. Code that
  called `Encoder::encode(...)` on 0.1.x must either enable `features = ["alloc"]`
  or migrate to `encode_into`. See [Migration](#migration-from-01x) below.
- `EncodeError` messages are now `&'static str` (no allocated `String`).

### Added

- Feature flags: `alloc` (owned output + SVG string), `std` (implies `alloc`),
  `image` (implies `std`, raster PNG/GIF/WebP output).
- Full-spec, scanner-verified rewrites of the larger symbologies:
  - **PDF417** (ISO/IEC 15438) — byte compaction + Reed–Solomon EC.
  - **GS1 DataBar Omnidirectional / RSS-14** (ISO/IEC 24724).
  - **Aztec Code** (ISO/IEC 24778) — Binary Shift, Reed–Solomon over
    GF(16/64/256/1024).
  - **USPS Intelligent Mail (IMb)** — verified bit-for-bit against the canonical
    USPS-B-3200 DAFT reference vector.
  - **Royal Mail RM4SCC** — 4-state 3-row output.
- Streaming SVG rendering into any `core::fmt::Write` sink via `common::svg`.

### Fixed

- **Data Matrix** ECC 200 now produces scannable symbols, with 32×32–48×48
  multi-region support for larger data.
- **EAN-13/EAN-8 L-code**, **UPC-E parity**, and the **Code 39** pattern table
  corrected (symbols now scan).
- **GS1-128** now decodes correctly (Code B path).
- **UPC-A / UPC-E check digit** for odd-length data (also released as 0.1.3).

### Packaging

- `examples/` and locally generated barcodes are excluded from the published
  crate.

## [0.1.3] — 2026-07-07

### Fixed

- **UPC-A / UPC-E check digit.** The shared check-digit routine weighted digits
  from the left, which is only correct for even-length data (EAN-13's 12
  digits). For UPC-A and UPC-E (11 data digits) the rightmost digit received the
  wrong weight, producing an invalid check digit — UPC-A symbols failed to scan.
  It now weights from the right (the length-independent GS1 rule); EAN-13/EAN-8
  output is unchanged.

## [0.1.2] — 2026

### Fixed

- Critical **EAN-13 / EAN-8 / UPC-E** encoding fixes (L-code digits and UPC-E
  parity) so retail symbols scan.
- **GS1-128** decoding correctness.

## [0.1.1] — 2026

### Fixed

- Data Matrix capacity/length handling and scannability improvements.

## [0.1.0] — 2026

- Initial release: QR, EAN-13/8, UPC-A/E, Code 128/39/93, Codabar, ITF, GS1-128,
  GS1 DataBar, PDF417, Data Matrix, Aztec, USPS IMb, Royal Mail RM4SCC.

## Migration from 0.1.x

**0.1.x (owned output):**

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::ean13::Ean13;

let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
```

**0.2.0, option A — keep the convenience API** (enable `alloc`):

```toml
barcodes = { version = "0.2", features = ["alloc"] }
```

```rust
// identical code — encode() and to_svg_string() require the `alloc` feature
let svg = Ean13::encode("5901234123457").unwrap().to_svg_string();
```

**0.2.0, option B — zero allocation** (default, no features):

```rust
use barcodes::common::traits::BarcodeEncoder;
use barcodes::common::types::Encoded;
use barcodes::ean_upc::ean13::Ean13;

let mut buf = [false; 128]; // one bool per module
let Encoded::Linear { len, .. } = Ean13::encode_into("5901234123457", &mut buf).unwrap()
else { unreachable!() };
let bars = &buf[..len]; // true = dark module
```

[0.2.0]: https://github.com/ashaffah/barcodes/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/ashaffah/barcodes/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ashaffah/barcodes/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ashaffah/barcodes/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ashaffah/barcodes/releases/tag/v0.1.0
