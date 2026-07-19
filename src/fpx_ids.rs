//! FlashPix property IDs used by fpx-convert, confirmed against the
//! `libfpx` reference implementation (`fpx/fpxutils.h`) and cross-checked
//! byte-for-byte against a real sample file. Only the properties this tool
//! actually reads are named here — FlashPix defines many more (per-path
//! decimation settings, ICC profiles, camera exposure settings, ...) that
//! are out of scope for this converter.

/// `Image Contents` property: shared JPEG quantization/Huffman tables for
/// the resolution whose tiles were encoded against table group
/// `compress_index` (a tile records which group it uses in the top byte of
/// its `compressionSubtype` field — see `subimage_header`). VT_BLOB.
pub fn pid_jpeg_tables(compress_index: u8) -> u32 {
    0x0300_0001 | ((compress_index as u32) << 16)
}

/// `Image Info` property: camera model, e.g. "DC210 Zoom (V01.02)". VT_LPWSTR.
pub const PID_CAMERA_MODEL: u32 = 0x2400_0001;

/// `Image Info` property: when the photo was captured. VT_FILETIME.
pub const PID_CAPTURE_DATE: u32 = 0x2500_0000;
