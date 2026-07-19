//! Encodes a decoded image as a JPEG, with the same archival EXIF payload
//! `exif.rs` builds for PNG embedded as a JPEG APP1 `Exif` segment instead
//! of a PNG `eXIf` chunk. Uses the `jpeg-encoder` crate: pure Rust, no C
//! toolchain needed for either build target, matching the constraint that
//! shaped the PNG encoder choice (see `png_writer.rs`).

use std::io::Write;

use jpeg_encoder::{ColorType, Encoder};

use crate::error::{FpxError, Result};
use crate::parse::DecodedImage;

/// Matches common photo-viewer defaults: visually close to the source for
/// a photo that's already been through one lossy JPEG generation in-camera,
/// without the size cost of a near-100 setting.
const QUALITY: u8 = 90;

pub fn write<W: Write>(writer: W, image: &DecodedImage) -> Result<()> {
    let width = u16::try_from(image.width).map_err(|_| FpxError::ImageTooLargeForJpeg {
        width: image.width,
        height: image.height,
    })?;
    let height = u16::try_from(image.height).map_err(|_| FpxError::ImageTooLargeForJpeg {
        width: image.width,
        height: image.height,
    })?;

    let mut encoder = Encoder::new(writer, QUALITY);
    if let Some(exif) = &image.exif {
        encoder.add_exif_metadata(exif)?;
    }
    encoder.encode(&image.rgb, width, height, ColorType::Rgb)?;
    Ok(())
}
