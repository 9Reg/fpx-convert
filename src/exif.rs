//! Builds the raw bytes for a PNG `eXIf` chunk: a minimal little-endian
//! TIFF structure holding just the two fields the spec requires archived —
//! camera model (IFD0 tag 0x0110) and capture date (Exif SubIFD tag
//! 0x9003, `DateTimeOriginal`) — linked from IFD0 via an ExifIFD pointer
//! (tag 0x8769) when a capture date is present.
//!
//! A PNG `eXIf` chunk's payload is exactly this TIFF structure (starting
//! with the "II"/"MM" byte-order marker) — unlike a JPEG APP1 segment, it
//! carries no leading `"Exif\0\0"` marker.

const TAG_MODEL: u16 = 0x0110;
const TAG_EXIF_IFD_POINTER: u16 = 0x8769;
const TAG_DATE_TIME_ORIGINAL: u16 = 0x9003;
const TYPE_ASCII: u16 = 2;
const TYPE_LONG: u16 = 4;

struct Entry {
    tag: u16,
    field_type: u16,
    count: u32,
    /// Raw field value, `count * type_size` bytes. Inlined into the IFD
    /// entry if it fits in 4 bytes, otherwise stored in the IFD's overflow
    /// area and referenced by offset.
    data: Vec<u8>,
}

impl Entry {
    fn ascii(tag: u16, s: &str) -> Self {
        let mut data = s.as_bytes().to_vec();
        data.push(0);
        Entry {
            tag,
            field_type: TYPE_ASCII,
            count: data.len() as u32,
            data,
        }
    }

    fn long(tag: u16, value: u32) -> Self {
        Entry {
            tag,
            field_type: TYPE_LONG,
            count: 1,
            data: value.to_le_bytes().to_vec(),
        }
    }
}

/// Serializes one IFD (its fixed-size entry table plus a trailing
/// next-IFD offset) and the overflow area for entries whose value doesn't
/// fit inline. `ifd_offset` is this IFD's absolute offset in the final
/// buffer, needed to compute overflow-area offsets.
fn write_ifd(entries: &[Entry], ifd_offset: u32, next_ifd_offset: u32) -> (Vec<u8>, Vec<u8>) {
    let fixed_len = 2 + entries.len() * 12 + 4;
    let mut fixed = Vec::with_capacity(fixed_len);
    fixed.extend_from_slice(&(entries.len() as u16).to_le_bytes());

    let mut overflow = Vec::new();
    let overflow_base = ifd_offset + fixed_len as u32;
    for entry in entries {
        fixed.extend_from_slice(&entry.tag.to_le_bytes());
        fixed.extend_from_slice(&entry.field_type.to_le_bytes());
        fixed.extend_from_slice(&entry.count.to_le_bytes());
        if entry.data.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..entry.data.len()].copy_from_slice(&entry.data);
            fixed.extend_from_slice(&inline);
        } else {
            let offset = overflow_base + overflow.len() as u32;
            fixed.extend_from_slice(&offset.to_le_bytes());
            overflow.extend_from_slice(&entry.data);
            if overflow.len() % 2 != 0 {
                overflow.push(0); // keep the next entry's offset word-aligned
            }
        }
    }
    fixed.extend_from_slice(&next_ifd_offset.to_le_bytes());
    (fixed, overflow)
}

/// Builds a PNG `eXIf` chunk payload from the camera model and/or a
/// pre-formatted `"YYYY:MM:DD HH:MM:SS"` capture date. Returns `None` if
/// both are absent (nothing to write, so no chunk should be emitted).
pub fn build(model: Option<&str>, date_time_original: Option<&str>) -> Option<Vec<u8>> {
    if model.is_none() && date_time_original.is_none() {
        return None;
    }

    let mut ifd0_entries = Vec::new();
    if let Some(model) = model {
        ifd0_entries.push(Entry::ascii(TAG_MODEL, model));
    }
    let exif_ptr_index = date_time_original.map(|_| {
        ifd0_entries.push(Entry::long(TAG_EXIF_IFD_POINTER, 0)); // patched below
        ifd0_entries.len() - 1
    });

    const IFD0_OFFSET: u32 = 8;
    let (mut ifd0_fixed, ifd0_overflow) = write_ifd(&ifd0_entries, IFD0_OFFSET, 0);

    let mut out = Vec::new();
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&IFD0_OFFSET.to_le_bytes());

    match (exif_ptr_index, date_time_original) {
        (Some(entry_index), Some(date_time)) => {
            let exif_ifd_offset =
                IFD0_OFFSET + ifd0_fixed.len() as u32 + ifd0_overflow.len() as u32;
            // Patch the ExifIFD-pointer entry's inline value now that we
            // know where the SubIFD will land: 2 (count) + entry*12 bytes
            // in, +8 to skip that entry's tag/type/count to its value.
            let value_pos = 2 + entry_index * 12 + 8;
            ifd0_fixed[value_pos..value_pos + 4].copy_from_slice(&exif_ifd_offset.to_le_bytes());

            out.extend_from_slice(&ifd0_fixed);
            out.extend_from_slice(&ifd0_overflow);

            let exif_entries = [Entry::ascii(TAG_DATE_TIME_ORIGINAL, date_time)];
            let (exif_fixed, exif_overflow) = write_ifd(&exif_entries, exif_ifd_offset, 0);
            out.extend_from_slice(&exif_fixed);
            out.extend_from_slice(&exif_overflow);
        }
        _ => {
            out.extend_from_slice(&ifd0_fixed);
            out.extend_from_slice(&ifd0_overflow);
        }
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_nothing_to_write() {
        assert!(build(None, None).is_none());
    }

    #[test]
    fn model_only_has_valid_tiff_header_and_contains_the_string() {
        let bytes = build(Some("DC210 Zoom (V01.02)"), None).unwrap();
        assert_eq!(&bytes[0..2], b"II");
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 42);
        assert!(bytes.windows(20).any(|w| w == b"DC210 Zoom (V01.02)\0"));
    }

    #[test]
    fn model_and_date_both_present() {
        let bytes = build(Some("DC210 Zoom (V01.02)"), Some("1997:12:25 15:29:39")).unwrap();
        // IFD0 offset (bytes 4..8) should be 8.
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 8);
        // IFD0 entry count.
        let entry_count = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        assert_eq!(entry_count, 2);
    }
}
