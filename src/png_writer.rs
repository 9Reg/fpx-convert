//! Encodes a decoded image as a lossless PNG, with an optional `eXIf`
//! chunk carrying archival metadata. Uses the `png` crate directly (rather
//! than a higher-level image library) because it's the pure-Rust encoder
//! the spec calls for — no C toolchain needed for either build target —
//! and because it exposes `eXIf` chunk writing as first-class support.

use std::borrow::Cow;
use std::io::Write;

use crate::error::Result;
use crate::parse::DecodedImage;

pub fn write<W: Write>(writer: W, image: &DecodedImage) -> Result<()> {
    let mut info = png::Info::with_size(image.width, image.height);
    info.color_type = png::ColorType::Rgb;
    info.bit_depth = png::BitDepth::Eight;
    if let Some(exif) = &image.exif {
        info.exif_metadata = Some(Cow::Borrowed(exif.as_slice()));
    }

    let encoder = png::Encoder::with_info(writer, info)?;
    let mut png_writer = encoder.write_header()?;
    png_writer.write_image_data(&image.rgb)?;
    Ok(())
}
