//! Decodes a resolution's tiles into one full RGB pixel buffer.
//!
//! Tiles are JPEG-compressed but, when they share a table group, don't
//! carry their own quantization/Huffman tables (see `image_contents`).
//! What's on disk for such a tile is a *second*, headerless JPEG stream:
//! `SOI SOF SOS <entropy-coded data> EOI` with no `DQT`/`DHT` segments.
//! libfpx's own decoder handles this by concatenating the shared
//! tables-only stream with the tile stream and feeding both `SOI`/`EOI`
//! pairs straight through a marker parser that tolerates the duplication.
//! A general-purpose JPEG decoder (`jpeg-decoder`, used here) doesn't
//! tolerate that, so instead this module splices the two into one
//! ordinary, valid JPEG before decoding: `SOI` + shared `DQT`/`DHT`
//! segments + the tile's own `SOF`/`SOS`/entropy-coded data/`EOI`. This
//! reconstructs exactly the byte sequence libfpx itself writes for a
//! self-contained (non-shared-table) tile, which is why any standard
//! decoder can read it. Verified end-to-end against a real sample file.

use std::collections::HashMap;
use std::io::Cursor;

use crate::error::{FpxError, Result};
use crate::subimage_header::{COMPRESS_TYPE_JPEG, SubimageHeader};

/// Splices a shared JPEG tables-only stream (`SOI DQT.. DHT.. EOI`) with a
/// tile's headerless JPEG stream (`SOI SOF SOS <data> EOI`) into one
/// ordinary JPEG (`SOI DQT.. DHT.. SOF SOS <data> EOI`).
fn splice_jpeg(tables: &[u8], tile: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + tables.len() + tile.len());
    out.extend_from_slice(&[0xFF, 0xD8]); // SOI
    if tables.len() > 4 {
        out.extend_from_slice(&tables[2..tables.len() - 2]); // drop tables' own SOI/EOI
    }
    out.extend_from_slice(&tile[2..]); // drop tile's own SOI, keep its EOI
    out
}

/// Decodes every tile in `header` and assembles them into one
/// `width * height * 3` RGB byte buffer. `subimage_data` is the matching
/// `Subimage 0000 Data` stream's content (28-byte FPX prefix already
/// stripped). `jpeg_tables` maps each shared-tables group index (as found
/// in a tile's `compressionSubtype`) to that group's tables-only JPEG blob.
pub fn decode_resolution(
    stream_name: &str,
    subimage_data: &[u8],
    header: &SubimageHeader,
    jpeg_tables: &HashMap<u8, Vec<u8>>,
) -> Result<Vec<u8>> {
    let width = header.width as usize;
    let height = header.height as usize;
    let tile_width = header.tile_width as usize;
    let tile_height = header.tile_height as usize;
    let tiles_per_row = width.div_ceil(tile_width);

    let mut full = vec![0u8; width * height * 3];

    for (index, tile) in header.tiles.iter().enumerate() {
        if tile.compress_type != COMPRESS_TYPE_JPEG {
            return Err(FpxError::UnsupportedCompression {
                stream: stream_name.to_string(),
                found: tile.compress_type,
            });
        }

        let tile_end = tile.offset + tile.size;
        let tile_bytes =
            subimage_data
                .get(tile.offset..tile_end)
                .ok_or_else(|| FpxError::Truncated {
                    stream: stream_name.to_string(),
                    needed: tile_end,
                    found: subimage_data.len(),
                })?;

        let table_index = tile.jpeg_tables_index();
        let spliced;
        let jpeg_bytes: &[u8] = if table_index == 0 {
            tile_bytes
        } else {
            let tables = jpeg_tables.get(&table_index).ok_or_else(|| {
                FpxError::MissingEntry(format!(
                    "tile {index} in '{stream_name}' references JPEG-tables group {table_index}, which is missing from 'Image Contents'"
                ))
            })?;
            spliced = splice_jpeg(tables, tile_bytes);
            &spliced
        };

        let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(jpeg_bytes));
        let pixels = decoder.decode().map_err(|source| FpxError::TileDecode {
            stream: stream_name.to_string(),
            index,
            source,
        })?;
        let info = decoder
            .info()
            .expect("decoder info available after successful decode");
        if info.pixel_format != jpeg_decoder::PixelFormat::RGB24 {
            return Err(FpxError::UnexpectedPixelFormat {
                stream: stream_name.to_string(),
                index,
                found: info.pixel_format,
            });
        }

        let col = index % tiles_per_row;
        let row = index / tiles_per_row;
        let x0 = col * tile_width;
        let y0 = row * tile_height;
        let crop_width = tile_width.min(width.saturating_sub(x0));
        let crop_height = tile_height.min(height.saturating_sub(y0));

        for ty in 0..crop_height {
            let src_start = (ty * tile_width) * 3;
            let dst_start = ((y0 + ty) * width + x0) * 3;
            full[dst_start..dst_start + crop_width * 3]
                .copy_from_slice(&pixels[src_start..src_start + crop_width * 3]);
        }
    }

    Ok(full)
}
