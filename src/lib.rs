//! fpx-convert: parses a FlashPix (`.fpx`) image and converts it to a
//! lossless PNG a modern browser can display directly. See
//! `specs/0001-fpx-conversion-pipeline.md` for the behavioral spec this
//! crate implements.

pub mod error;
pub mod parse;

mod cfbf;
mod exif;
mod filetime;
mod fpx_ids;
mod png_writer;
mod propset;
mod subimage_header;
mod tile_decode;

use std::io::Write;

use error::Result;

/// Runs the whole parse-then-convert pipeline: given a `.fpx` file's raw
/// bytes, writes the converted PNG to `writer`. Both CLI modes (file-path
/// and stdin/stdout) go through this same entry point.
pub fn convert<W: Write>(input: &[u8], writer: W) -> Result<()> {
    let image = parse::parse_and_decode(input)?;
    png_writer::write(writer, &image)
}
