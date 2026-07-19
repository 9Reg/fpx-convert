//! Integration tests for the parse stage's error handling, using
//! synthetic CFBF fixtures built with the `cfb` crate's write support.
//! These cover cases the one real sample file (gitignored, not available
//! in CI) can't exercise: malformed or non-FlashPix containers.

use std::io::{Cursor, Write};

use fpx_convert::error::FpxError;

/// Builds a minimal, valid OLE property-set stream with zero properties —
/// enough for `PropertySet::parse` to succeed without needing any actual
/// property data, per the layout documented in `src/propset.rs`.
fn empty_property_set_stream() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&0xFFFEu16.to_le_bytes()); // byte order mark
    buf.extend_from_slice(&0u16.to_le_bytes()); // format
    buf.extend_from_slice(&0u32.to_le_bytes()); // os version (unused)
    buf.extend_from_slice(&[0u8; 16]); // clsid (unused)
    buf.extend_from_slice(&1u32.to_le_bytes()); // section count
    buf.extend_from_slice(&[0u8; 16]); // FORMATIDOFFSET.format_id (unused)
    buf.extend_from_slice(&48u32.to_le_bytes()); // FORMATIDOFFSET.section_offset
    debug_assert_eq!(buf.len(), 48);
    buf.extend_from_slice(&8u32.to_le_bytes()); // section size (unused)
    buf.extend_from_slice(&0u32.to_le_bytes()); // property count
    buf
}

#[test]
fn non_cfbf_input_is_rejected() {
    let err = fpx_convert::parse::parse_and_decode(b"not a compound file at all").unwrap_err();
    assert!(
        matches!(err, FpxError::NotCompoundFile(_)),
        "expected NotCompoundFile, got {err:?}"
    );
}

#[test]
fn truncated_cfbf_input_is_rejected() {
    // A CFBF signature with nothing behind it should fail cleanly rather
    // than panicking on out-of-bounds reads.
    let err =
        fpx_convert::parse::parse_and_decode(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
            .unwrap_err();
    assert!(
        matches!(err, FpxError::NotCompoundFile(_)),
        "expected NotCompoundFile, got {err:?}"
    );
}

#[test]
fn cfbf_without_image_contents_is_rejected() {
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    cf.create_stream("/Unrelated")
        .unwrap()
        .write_all(b"hi")
        .unwrap();
    let bytes = cf.into_inner().into_inner();

    let err = fpx_convert::parse::parse_and_decode(&bytes).unwrap_err();
    assert!(
        matches!(err, FpxError::NotFlashPix),
        "expected NotFlashPix, got {err:?}"
    );
}

#[test]
fn flashpix_file_with_no_resolutions_is_rejected() {
    let mut cf = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    cf.create_storage("/Data Object Store 000001").unwrap();
    cf.create_stream("/Data Object Store 000001/\u{5}Image Contents")
        .unwrap()
        .write_all(&empty_property_set_stream())
        .unwrap();
    let bytes = cf.into_inner().into_inner();

    let err = fpx_convert::parse::parse_and_decode(&bytes).unwrap_err();
    assert!(
        matches!(err, FpxError::NoResolutions),
        "expected NoResolutions, got {err:?}"
    );
}
