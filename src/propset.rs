//! Generic OLE property-set stream parser (the on-disk format behind
//! `SummaryInformation`, `Image Contents`, and `Image Info` streams in a
//! FlashPix file — this is Microsoft's general-purpose "structured storage
//! property set" format, not anything FlashPix-specific).
//!
//! Layout, confirmed field-for-field against a real sample file:
//!
//! ```text
//! offset  0: PROPERTYSETHEADER (28 bytes)
//!             u16 byte_order (always 0xFFFE)
//!             u16 format (always 0)
//!             u32 os_version
//!             [u8; 16] clsid
//!             u32 section_count
//! offset 28: FORMATIDOFFSET[section_count] (20 bytes each)
//!             [u8; 16] format_id
//!             u32 section_offset   -- absolute, from start of this stream
//! at section_offset: PROPERTYSECTIONHEADER
//!             u32 section_size
//!             u32 property_count
//!             PROPERTYIDOFFSET[property_count] (8 bytes each)
//!               u32 property_id
//!               u32 value_offset   -- relative to section_offset
//! at section_offset + value_offset: SERIALIZEDPROPERTYVALUE
//!             u32 vt_type
//!             ...type-specific payload
//! ```
//!
//! FlashPix always writes exactly one section, so this parser only looks at
//! the first one. Property ID 0 is reserved for a name dictionary (used for
//! named, rather than numeric, properties); FlashPix's own well-known
//! properties are all numeric, so entries with ID 0 are simply skipped.

use std::collections::HashMap;

use crate::error::{FpxError, Result};

const HEADER_LEN: usize = 28;
const BYTE_ORDER_MARK: u16 = 0xFFFE;
const FORMAT_ID_OFFSET_LEN: usize = 20;
const PROPERTY_ID_OFFSET_LEN: usize = 8;

const VT_LPWSTR: u32 = 31;
const VT_FILETIME: u32 = 64;
const VT_BLOB: u32 = 65;

fn u16le(stream: &str, data: &[u8], offset: usize) -> Result<u16> {
    let end = offset + 2;
    data.get(offset..end)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| FpxError::Truncated {
            stream: stream.to_string(),
            needed: end,
            found: data.len(),
        })
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

/// A parsed property set: just enough to look up specific well-known
/// properties by ID and decode their value as one of the handful of VT_*
/// types FlashPix actually uses. Properties are decoded lazily, on lookup,
/// rather than eagerly into a generic value enum, since the caller always
/// knows in advance which property and which type it expects.
#[derive(Debug)]
pub struct PropertySet<'a> {
    stream_name: String,
    data: &'a [u8],
    /// property ID -> absolute offset (into `data`) of its SERIALIZEDPROPERTYVALUE
    values: HashMap<u32, usize>,
}

impl<'a> PropertySet<'a> {
    pub fn parse(stream_name: &str, data: &'a [u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(FpxError::Truncated {
                stream: stream_name.to_string(),
                needed: HEADER_LEN,
                found: data.len(),
            });
        }
        let byte_order = u16le(stream_name, data, 0)?;
        if byte_order != BYTE_ORDER_MARK {
            return Err(FpxError::MissingEntry(format!(
                "'{stream_name}' is not a valid OLE property-set stream (bad byte-order marker)"
            )));
        }
        let section_count = u32le(stream_name, data, 24)? as usize;
        if section_count == 0 {
            return Err(FpxError::MissingEntry(format!(
                "'{stream_name}' has no property sections"
            )));
        }

        // Only the first section is used — FlashPix always writes exactly one.
        let format_id_offset_pos = HEADER_LEN;
        if data.len() < format_id_offset_pos + FORMAT_ID_OFFSET_LEN {
            return Err(FpxError::Truncated {
                stream: stream_name.to_string(),
                needed: format_id_offset_pos + FORMAT_ID_OFFSET_LEN,
                found: data.len(),
            });
        }
        let section_offset = u32le(stream_name, data, format_id_offset_pos + 16)? as usize;

        if data.len() < section_offset + 8 {
            return Err(FpxError::Truncated {
                stream: stream_name.to_string(),
                needed: section_offset + 8,
                found: data.len(),
            });
        }
        let property_count = u32le(stream_name, data, section_offset + 4)? as usize;

        let table_start = section_offset + 8;
        let table_len = property_count * PROPERTY_ID_OFFSET_LEN;
        if data.len() < table_start + table_len {
            return Err(FpxError::Truncated {
                stream: stream_name.to_string(),
                needed: table_start + table_len,
                found: data.len(),
            });
        }

        let mut values = HashMap::with_capacity(property_count);
        for i in 0..property_count {
            let base = table_start + i * PROPERTY_ID_OFFSET_LEN;
            let property_id = u32le(stream_name, data, base)?;
            if property_id == 0 {
                continue; // name dictionary; unused by FlashPix's own properties
            }
            let relative_offset = u32le(stream_name, data, base + 4)? as usize;
            values.insert(property_id, section_offset + relative_offset);
        }

        Ok(Self {
            stream_name: stream_name.to_string(),
            data,
            values,
        })
    }

    fn value_offset(&self, id: u32, name: &'static str) -> Result<usize> {
        self.values
            .get(&id)
            .copied()
            .ok_or(FpxError::MissingProperty {
                stream: self.stream_name.clone(),
                property: id,
                name,
            })
    }

    fn check_type(
        &self,
        offset: usize,
        id: u32,
        expected: u32,
        expected_name: &'static str,
    ) -> Result<()> {
        let found = u32le(&self.stream_name, self.data, offset)?;
        if found != expected {
            return Err(FpxError::WrongPropertyType {
                stream: self.stream_name.clone(),
                property: id,
                expected: expected_name,
                found,
            });
        }
        Ok(())
    }
    /// Reads a VT_LPWSTR (length-prefixed UTF-16LE string) property. The
    /// stored character count includes the terminating NUL, which is
    /// stripped from the returned string.
    pub fn get_lpwstr(&self, id: u32, name: &'static str) -> Result<String> {
        let offset = self.value_offset(id, name)?;
        self.check_type(offset, id, VT_LPWSTR, "VT_LPWSTR")?;
        let char_count = u32le(&self.stream_name, self.data, offset + 4)? as usize;
        let byte_len = char_count * 2;
        let start = offset + 8;
        let end = start + byte_len;
        let bytes = self
            .data
            .get(start..end)
            .ok_or_else(|| FpxError::Truncated {
                stream: self.stream_name.clone(),
                needed: end,
                found: self.data.len(),
            })?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = String::from_utf16_lossy(&units);
        Ok(s.trim_end_matches('\0').to_string())
    }

    /// Reads a VT_FILETIME property as raw 100ns ticks since 1601-01-01 UTC.
    pub fn get_filetime(&self, id: u32, name: &'static str) -> Result<u64> {
        let offset = self.value_offset(id, name)?;
        self.check_type(offset, id, VT_FILETIME, "VT_FILETIME")?;
        let low = u32le(&self.stream_name, self.data, offset + 4)? as u64;
        let high = u32le(&self.stream_name, self.data, offset + 8)? as u64;
        Ok((high << 32) | low)
    }

    /// Reads a VT_BLOB property as a byte slice.
    pub fn get_blob(&self, id: u32, name: &'static str) -> Result<&'a [u8]> {
        let offset = self.value_offset(id, name)?;
        self.check_type(offset, id, VT_BLOB, "VT_BLOB")?;
        let size = u32le(&self.stream_name, self.data, offset + 4)? as usize;
        let start = offset + 8;
        let end = start + size;
        self.data
            .get(start..end)
            .ok_or_else(|| FpxError::Truncated {
                stream: self.stream_name.clone(),
                needed: end,
                found: self.data.len(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic property-set stream, in the same layout
    /// `PropertySet::parse` expects, for exercising the parser without a
    /// real `.fpx` sample on hand.
    struct Builder {
        properties: Vec<(u32, Vec<u8>)>,
    }

    impl Builder {
        fn new() -> Self {
            Self {
                properties: Vec::new(),
            }
        }

        fn add(mut self, id: u32, value: Vec<u8>) -> Self {
            self.properties.push((id, value));
            self
        }

        fn build(self) -> Vec<u8> {
            let mut buf = Vec::new();
            buf.extend_from_slice(&0xFFFEu16.to_le_bytes());
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&0u32.to_le_bytes());
            buf.extend_from_slice(&[0u8; 16]);
            buf.extend_from_slice(&1u32.to_le_bytes());
            buf.extend_from_slice(&[0u8; 16]);
            const SECTION_OFFSET: u32 = 48;
            buf.extend_from_slice(&SECTION_OFFSET.to_le_bytes());
            assert_eq!(buf.len(), SECTION_OFFSET as usize);

            let table_start = 8u32; // relative to section start: size(4) + count(4)
            let table_len = self.properties.len() as u32 * 8;
            let mut relative_offset = table_start + table_len;
            let mut table = Vec::new();
            let mut values = Vec::new();
            for (id, value) in &self.properties {
                table.extend_from_slice(&id.to_le_bytes());
                table.extend_from_slice(&relative_offset.to_le_bytes());
                values.extend_from_slice(value);
                relative_offset += value.len() as u32;
            }

            buf.extend_from_slice(&(table_start + table_len + values.len() as u32).to_le_bytes());
            buf.extend_from_slice(&(self.properties.len() as u32).to_le_bytes());
            buf.extend_from_slice(&table);
            buf.extend_from_slice(&values);
            buf
        }
    }

    fn vt_lpwstr(s: &str) -> Vec<u8> {
        let mut units: Vec<u16> = s.encode_utf16().collect();
        units.push(0);
        let mut v = Vec::new();
        v.extend_from_slice(&VT_LPWSTR.to_le_bytes());
        v.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for u in units {
            v.extend_from_slice(&u.to_le_bytes());
        }
        v
    }

    fn vt_filetime(ticks: u64) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_FILETIME.to_le_bytes());
        v.extend_from_slice(&(ticks as u32).to_le_bytes());
        v.extend_from_slice(&((ticks >> 32) as u32).to_le_bytes());
        v
    }

    fn vt_blob(bytes: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&VT_BLOB.to_le_bytes());
        v.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        v.extend_from_slice(bytes);
        v
    }

    #[test]
    fn reads_lpwstr_filetime_and_blob() {
        let stream = Builder::new()
            .add(0x2400_0001, vt_lpwstr("DC210 Zoom (V01.02)"))
            .add(0x2500_0000, vt_filetime(0x01BD_1149_E9DB_7380))
            .add(0x0301_0001, vt_blob(&[0xFF, 0xD8, 0xFF, 0xDB, 1, 2, 3]))
            .build();
        let ps = PropertySet::parse("test", &stream).unwrap();
        assert_eq!(
            ps.get_lpwstr(0x2400_0001, "Model").unwrap(),
            "DC210 Zoom (V01.02)"
        );
        assert_eq!(
            ps.get_filetime(0x2500_0000, "CaptureDate").unwrap(),
            0x01BD_1149_E9DB_7380
        );
        assert_eq!(
            ps.get_blob(0x0301_0001, "JPEGTables").unwrap(),
            &[0xFF, 0xD8, 0xFF, 0xDB, 1, 2, 3][..]
        );
    }

    #[test]
    fn missing_property_is_a_clear_error() {
        let stream = Builder::new().build();
        let ps = PropertySet::parse("test", &stream).unwrap();
        let err = ps.get_lpwstr(0x2400_0001, "Model").unwrap_err();
        assert!(matches!(err, FpxError::MissingProperty { .. }));
    }

    #[test]
    fn wrong_type_is_a_clear_error() {
        let stream = Builder::new().add(0x2400_0001, vt_filetime(0)).build();
        let ps = PropertySet::parse("test", &stream).unwrap();
        let err = ps.get_lpwstr(0x2400_0001, "Model").unwrap_err();
        assert!(matches!(err, FpxError::WrongPropertyType { .. }));
    }

    #[test]
    fn truncated_stream_is_rejected() {
        let err = PropertySet::parse("test", &[0xFE, 0xFF, 0, 0]).unwrap_err();
        assert!(matches!(err, FpxError::Truncated { .. }));
    }

    #[test]
    fn bad_byte_order_mark_is_rejected() {
        let mut bad = Builder::new().build();
        bad[0] = 0;
        bad[1] = 0;
        let err = PropertySet::parse("test", &bad).unwrap_err();
        assert!(matches!(err, FpxError::MissingEntry(_)));
    }
}
