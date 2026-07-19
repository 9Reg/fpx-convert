//! Thin helpers on top of the `cfb` crate (which implements the generic
//! Compound File Binary / OLE2 structured-storage container format) for the
//! specific things fpx-convert needs: locating the "object" storage that
//! holds a FlashPix image's streams, listing its `Resolution NNNN`
//! sub-storages, and reading whole streams into memory.
//!
//! FlashPix files nest the actual image data one level down, inside a
//! `Data Object Store NNNNNN` storage (confirmed against a real sample file
//! captured with a Kodak DC210 Zoom) rather than at the container root, so
//! we locate that storage by searching for its telltale `Image Contents`
//! stream rather than assuming a fixed path.

use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use crate::error::{FpxError, Result};

/// Every FPX-specific stream (Subimage Header/Data, and the top-level
/// property-set streams) is preceded by this 28-byte header before its
/// real content. For property-set streams the header doubles as the
/// property set's own `PROPERTYSETHEADER`; for Subimage Header/Data
/// streams it's purely a prefix to skip.
pub const FPX_STREAM_PREFIX_LEN: usize = 28;

/// OLE property-set streams (`Image Contents`, `Image Info`,
/// `SummaryInformation`) are conventionally stored with a leading control
/// character — confirmed against a real sample file to be U+0005 — that
/// doesn't appear anywhere in the spec's naming but is very much part of
/// the on-disk stream name. This matches an entry's real name against the
/// "visible" name the spec uses, tolerating that optional prefix.
fn matches_visible_name(actual: &str, visible: &str) -> bool {
    actual == visible
        || actual
            .strip_prefix(|c: char| (c as u32) < 0x20)
            .is_some_and(|rest| rest == visible)
}

/// Finds the storage that directly contains an `Image Contents` stream —
/// i.e. the root of the FlashPix "object" this file holds. Returns
/// `FpxError::NotFlashPix` if no such stream exists anywhere in the
/// container (the file is a valid CFBF container, but not a FlashPix one).
pub fn find_object_root<F: Read + Seek>(cf: &cfb::CompoundFile<F>) -> Result<PathBuf> {
    cf.walk()
        .find(|entry| !entry.is_storage() && matches_visible_name(entry.name(), "Image Contents"))
        .map(|entry| {
            entry
                .path()
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("/"))
        })
        .ok_or(FpxError::NotFlashPix)
}

/// Finds a stream by its "visible" name (see `matches_visible_name`) among
/// the direct children of `parent`.
pub fn find_stream<F: Read + Seek>(
    cf: &cfb::CompoundFile<F>,
    parent: &Path,
    visible_name: &str,
) -> Result<PathBuf> {
    let entries = cf
        .read_storage(parent)
        .map_err(|source| FpxError::StreamRead {
            name: parent.display().to_string(),
            source,
        })?;
    entries
        .filter(|entry| !entry.is_storage() && matches_visible_name(entry.name(), visible_name))
        .map(|entry| entry.path().to_path_buf())
        .next()
        .ok_or_else(|| FpxError::MissingEntry(format!("{}/{}", parent.display(), visible_name)))
}

/// Lists the `Resolution NNNN` storages directly under `object_root`, in
/// whatever order the container stores them (no particular meaning — see
/// the spec's note that the NNNN suffix does not indicate resolution rank).
pub fn list_resolution_storages<F: Read + Seek>(
    cf: &cfb::CompoundFile<F>,
    object_root: &Path,
) -> Result<Vec<PathBuf>> {
    let entries = cf
        .read_storage(object_root)
        .map_err(|source| FpxError::StreamRead {
            name: object_root.display().to_string(),
            source,
        })?;
    let storages: Vec<PathBuf> = entries
        .filter(|entry| entry.is_storage() && is_resolution_storage_name(entry.name()))
        .map(|entry| entry.path().to_path_buf())
        .collect();
    if storages.is_empty() {
        return Err(FpxError::NoResolutions);
    }
    Ok(storages)
}

fn is_resolution_storage_name(name: &str) -> bool {
    name.strip_prefix("Resolution ")
        .is_some_and(|suffix| suffix.len() == 4 && suffix.bytes().all(|b| b.is_ascii_digit()))
}

/// Reads an entire stream into memory. FPX images are small enough (the
/// format tops out around 1-2 megapixels) that buffering whole streams is
/// simpler than the alternative and not a real cost.
pub fn read_stream_full<F: Read + Seek>(
    cf: &mut cfb::CompoundFile<F>,
    path: &Path,
) -> Result<Vec<u8>> {
    let mut stream = cf
        .open_stream(path)
        .map_err(|source| FpxError::StreamRead {
            name: path.display().to_string(),
            source,
        })?;
    let mut buf = Vec::with_capacity(stream.len() as usize);
    stream
        .read_to_end(&mut buf)
        .map_err(|source| FpxError::StreamRead {
            name: path.display().to_string(),
            source,
        })?;
    Ok(buf)
}

/// Strips the 28-byte FPX stream prefix, returning the stream's real
/// content. Returns a clear error if the stream is too short to even hold
/// the prefix.
pub fn strip_fpx_prefix<'a>(stream_name: &str, data: &'a [u8]) -> Result<&'a [u8]> {
    if data.len() < FPX_STREAM_PREFIX_LEN {
        return Err(FpxError::Truncated {
            stream: stream_name.to_string(),
            needed: FPX_STREAM_PREFIX_LEN,
            found: data.len(),
        });
    }
    Ok(&data[FPX_STREAM_PREFIX_LEN..])
}
