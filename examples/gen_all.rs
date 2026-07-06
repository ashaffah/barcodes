//! Generate a PNG for every symbology so they can be tested with an online
//! decoder. Not part of the library; run with the `image` feature:
//!
//! ```sh
//! cargo run --example gen_all --features image
//! ```
//!
//! Output goes to `generated_barcodes/` (gitignored).

use barcodes::common::traits::BarcodeEncoder;
use barcodes::ean_upc::{ean8::Ean8, ean13::Ean13, upca::UpcA, upce::UpcE};
use barcodes::gs1::{databar::DataBar, gs1_128::Gs1_128};
use barcodes::linear::{
    codabar::Codabar, code39::Code39, code93::Code93, code128::Code128, itf::Itf,
};
use barcodes::postal::{imb::Imb, rm4scc::Rm4scc};
use barcodes::qrcode::{EncodeTextOptions, QrCode, QrCodeEcc, Version};
use barcodes::twod::{aztec::Aztec, datamatrix::DataMatrix, pdf417::Pdf417};

use image::{GrayImage, Luma};

const OUT_DIR: &str = "generated_barcodes";
const WHITE: Luma<u8> = Luma([255]);
const BLACK: Luma<u8> = Luma([0]);

/// Encode with a trait encoder and save a PNG.
fn save<E: BarcodeEncoder<Input = str>>(name: &str, data: &str, module: u32) {
    match E::encode(data) {
        Ok(out) => {
            let img = out.to_image(module);
            let path = format!("{OUT_DIR}/{name}.png");
            img.save(&path).unwrap();
            println!(
                "  OK   {name:<12} \"{data}\"  ({}x{})",
                img.width(),
                img.height()
            );
        }
        Err(e) => println!("  FAIL {name:<12} {e:?}"),
    }
}

/// QR uses its own API; render its modules to a PNG here.
fn save_qr(name: &str, data: &str, module: u32) {
    let mut outbuf = vec![0u8; Version::MAX.buffer_len()];
    let mut tmpbuf = vec![0u8; Version::MAX.buffer_len()];
    let qr = QrCode::encode_text(
        data,
        &mut tmpbuf,
        &mut outbuf,
        EncodeTextOptions {
            ecl: QrCodeEcc::Medium,
            minversion: Version::MIN,
            maxversion: Version::MAX,
            mask: None,
            boostecl: true,
        },
    )
    .unwrap();

    let quiet = 4u32;
    let size = qr.size() as u32;
    let dim = (size + 2 * quiet) * module;
    let mut img = GrayImage::from_pixel(dim, dim, WHITE);
    for y in 0..size {
        for x in 0..size {
            if qr.get_module(x as i32, y as i32) {
                for dy in 0..module {
                    for dx in 0..module {
                        img.put_pixel((x + quiet) * module + dx, (y + quiet) * module + dy, BLACK);
                    }
                }
            }
        }
    }
    let path = format!("{OUT_DIR}/{name}.png");
    img.save(&path).unwrap();
    println!("  OK   {name:<12} \"{data}\"  ({dim}x{dim})");
}

fn main() {
    std::fs::create_dir_all(OUT_DIR).unwrap();
    println!("Generating barcodes into {OUT_DIR}/ ...\n");

    println!("[Linear / retail]");
    save::<Ean13>("ean13", "5901234123457", 3);
    save::<Ean8>("ean8", "96385074", 3);
    save::<UpcA>("upca", "03600029145", 3); // 11 digits, check auto-computed
    save::<UpcE>("upce", "01234505", 3);
    save::<Code128>("code128", "Hello128", 2);
    save::<Code39>("code39", "CODE39", 2);
    save::<Code93>("code93", "CODE93", 2);
    save::<Codabar>("codabar", "40156", 2); // A/B start/stop added automatically
    save::<Itf>("itf", "1234567890", 2);

    println!("\n[GS1]");
    save::<Gs1_128>("gs1_128", "(01)01234567890128", 2);
    save::<DataBar>("databar", "2001234567890", 3);

    println!("\n[2D]");
    save_qr("qr_code", "https://crates.io/crates/barcodes", 4);
    save::<DataMatrix>("datamatrix", "Hello DataMatrix 2026", 5);
    save::<Pdf417>("pdf417", "PDF417 test payload — larger data works too!", 2);
    save::<Aztec>("aztec", "HELLO AZTEC 2026", 5);

    println!("\n[Postal / 4-state]");
    save::<Imb>("imb", "01234567094987654321-01234567891", 3);
    save::<Rm4scc>("rm4scc", "SN34RD1A", 3);

    println!("\nDone. Open the PNGs in {OUT_DIR}/ and drop them into an online decoder.");
}
