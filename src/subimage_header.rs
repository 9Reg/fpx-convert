//! Parser for a resolution's `Subimage 0000 Header` stream: the sub-image's
//! pixel dimensions, tile size, and per-tile table (offset/length/
//! compression into the matching `Subimage 0000 Data` stream).
//!
//! Layout, confirmed field-for-field against a real sample file (after
//! stripping the 28-byte FPX stream prefix — see `cfbf::strip_fpx_prefix`):
//!
//! ```text
//! offset  0: u32 length_of_this_fixed_header (always 36)
//! offset  4: u32 real_width
//! offset  8: u32 real_height
//! offset 12: u32 tile_count
//! offset 16: u32 tile_width
//! offset 20: u32 tile_height
//! offset 24: u32 channel_count
//! offset 28: u32 tile_table_offset   -- from the same base as above
//! offset 32: u32 tile_entry_size     -- bytes per tile-table entry (16)
//! at tile_table_offset: tile_count * TileEntry, tile_entry_size bytes each:
//!             u32 offset          -- into Subimage Data stream content
//!             u32 size            -- compressed byte length
//!             u32 compress_type
//!             u32 compress_subtype -- top byte is the shared JPEG-tables index
//! ```
//!
//! Tiles are stored in row-major order over a `ceil(width/tile_width) x
//! ceil(height/tile_height)` grid; edge tiles are encoded at the full tile
//! size and cropped to the real image bounds after decoding.

use crate::error::{FpxError, Result};

const FIXED_HEADER_LEN: usize = 36;

/// JPEG tile compression, confirmed against a real sample file (libfpx
/// normalizes all JPEG variants to this value on write).
pub const COMPRESS_TYPE_JPEG: u32 = 2;

#[derive(Debug, Clone, Copy)]
pub struct TileEntry {
    pub offset: usize,
    pub size: usize,
    pub compress_type: u32,
    pub compress_subtype: u32,
}

impl TileEntry {
    /// The shared JPEG-tables group this tile's compressed bytes were
    /// encoded against (0 means the tile is self-contained and carries its
    /// own quantization/Huffman tables).
    pub fn jpeg_tables_index(&self) -> u8 {
        (self.compress_subtype >> 24) as u8
    }
}

#[derive(Debug)]
pub struct SubimageHeader {
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles: Vec<TileEntry>,
}

fn u32le(stream: &str, data: &[u8], offset: usize) -> Result<u32> {
    let end = offset + 4;
    data.get(offset..end)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| FpxError::Truncated {
            stream: stream.to_string(),
            needed: end,
            found: data.len(),
        })
}

/// Parses a Subimage Header stream's content (with the 28-byte FPX prefix
/// already stripped).
pub fn parse(stream_name: &str, content: &[u8]) -> Result<SubimageHeader> {
    if content.len() < FIXED_HEADER_LEN {
        return Err(FpxError::Truncated {
            stream: stream_name.to_string(),
            needed: FIXED_HEADER_LEN,
            found: content.len(),
        });
    }

    let width = u32le(stream_name, content, 4)?;
    let height = u32le(stream_name, content, 8)?;
    let tile_count = u32le(stream_name, content, 12)? as usize;
    let tile_width = u32le(stream_name, content, 16)?;
    let tile_height = u32le(stream_name, content, 20)?;
    let table_offset = u32le(stream_name, content, 28)? as usize;
    let entry_size = u32le(stream_name, content, 32)? as usize;

    if width == 0 || height == 0 {
        return Err(FpxError::MissingEntry(format!(
            "'{stream_name}' reports a zero-sized image"
        )));
    }
    if tile_width == 0 || tile_height == 0 {
        return Err(FpxError::MissingEntry(format!(
            "'{stream_name}' reports a zero-sized tile"
        )));
    }
    if entry_size < 16 {
        return Err(FpxError::MissingEntry(format!(
            "'{stream_name}' has an unexpectedly small tile-table entry size ({entry_size} bytes)"
        )));
    }

    let mut tiles = Vec::with_capacity(tile_count);
    for i in 0..tile_count {
        let base = table_offset + i * entry_size;
        let needed_end = base + 16;
        if content.len() < needed_end {
            return Err(FpxError::Truncated {
                stream: stream_name.to_string(),
                needed: needed_end,
                found: content.len(),
            });
        }
        tiles.push(TileEntry {
            offset: u32le(stream_name, content, base)? as usize,
            size: u32le(stream_name, content, base + 4)? as usize,
            compress_type: u32le(stream_name, content, base + 8)?,
            compress_subtype: u32le(stream_name, content, base + 12)?,
        });
    }

    Ok(SubimageHeader {
        width,
        height,
        tile_width,
        tile_height,
        tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a Subimage Header stream's content (prefix already stripped)
    /// for a single-tile image, matching the on-disk layout confirmed
    /// against a real sample file.
    fn single_tile_header(width: u32, height: u32, tile_size: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&36u32.to_le_bytes()); // length_info
        buf.extend_from_slice(&width.to_le_bytes());
        buf.extend_from_slice(&height.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes()); // tile_count
        buf.extend_from_slice(&tile_size.to_le_bytes()); // tile_width
        buf.extend_from_slice(&tile_size.to_le_bytes()); // tile_height
        buf.extend_from_slice(&3u32.to_le_bytes()); // channel_count
        buf.extend_from_slice(&36u32.to_le_bytes()); // table_offset
        buf.extend_from_slice(&16u32.to_le_bytes()); // entry_size
        assert_eq!(buf.len(), 36);
        buf.extend_from_slice(&0u32.to_le_bytes()); // tile offset
        buf.extend_from_slice(&100u32.to_le_bytes()); // tile size
        buf.extend_from_slice(&COMPRESS_TYPE_JPEG.to_le_bytes());
        buf.extend_from_slice(&0x0101_2200u32.to_le_bytes()); // subtype, table index 1
        buf
    }

    #[test]
    fn parses_dimensions_and_tile_table() {
        let content = single_tile_header(100, 80, 64);
        let header = parse("test", &content).unwrap();
        assert_eq!(header.width, 100);
        assert_eq!(header.height, 80);
        assert_eq!(header.tile_width, 64);
        assert_eq!(header.tile_height, 64);
        assert_eq!(header.tiles.len(), 1);
        assert_eq!(header.tiles[0].offset, 0);
        assert_eq!(header.tiles[0].size, 100);
        assert_eq!(header.tiles[0].compress_type, COMPRESS_TYPE_JPEG);
        assert_eq!(header.tiles[0].jpeg_tables_index(), 1);
    }

    #[test]
    fn self_contained_tile_has_table_index_zero() {
        let mut content = single_tile_header(64, 64, 64);
        let last = content.len() - 4;
        content[last..].copy_from_slice(&0u32.to_le_bytes());
        let header = parse("test", &content).unwrap();
        assert_eq!(header.tiles[0].jpeg_tables_index(), 0);
    }

    #[test]
    fn truncated_header_is_rejected() {
        let err = parse("test", &[0u8; 10]).unwrap_err();
        assert!(matches!(err, FpxError::Truncated { .. }));
    }

    #[test]
    fn truncated_tile_table_is_rejected() {
        let mut content = single_tile_header(100, 80, 64);
        content.truncate(40); // fixed header intact, tile entry cut short
        let err = parse("test", &content).unwrap_err();
        assert!(matches!(err, FpxError::Truncated { .. }));
    }

    #[test]
    fn zero_sized_image_is_rejected() {
        let content = single_tile_header(0, 80, 64);
        let err = parse("test", &content).unwrap_err();
        assert!(matches!(err, FpxError::MissingEntry(_)));
    }
}
