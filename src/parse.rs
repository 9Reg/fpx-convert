//! Orchestrates the parse stage end to end: open the CFBF container, find
//! the FlashPix object inside it, pick the best available resolution,
//! decode its tiles, and collect archival metadata — producing one RGB
//! pixel buffer plus an optional EXIF payload, ready for the convert stage.

use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use crate::cfbf;
use crate::error::{FpxError, Result};
use crate::exif;
use crate::filetime;
use crate::fpx_ids;
use crate::propset::PropertySet;
use crate::subimage_header::{self, SubimageHeader};
use crate::tile_decode;

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// 8-bit RGB, row-major, `width * height * 3` bytes.
    pub rgb: Vec<u8>,
    /// Raw payload for a PNG `eXIf` chunk, if any archival metadata was
    /// found (a missing or incomplete `Image Info` stream is not an error
    /// — metadata preservation is best-effort, per the spec).
    pub exif: Option<Vec<u8>>,
}

impl std::fmt::Debug for DecodedImage {
    // Manual impl so debug output (e.g. from a failed `unwrap`) doesn't
    // dump millions of raw pixel bytes.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgb", &format_args!("<{} bytes>", self.rgb.len()))
            .field("exif", &self.exif)
            .finish()
    }
}

/// Parses and decodes a `.fpx` file's best available resolution from raw
/// input bytes (either a file's contents or piped stdin).
pub fn parse_and_decode(input: &[u8]) -> Result<DecodedImage> {
    let mut cf = cfb::CompoundFile::open(Cursor::new(input)).map_err(FpxError::NotCompoundFile)?;

    let object_root = cfbf::find_object_root(&cf)?;

    let jpeg_tables_source = read_image_contents(&mut cf, &object_root)?;

    let resolution_storages = cfbf::list_resolution_storages(&cf, &object_root)?;
    let (best_storage, header) = select_best_resolution(&mut cf, &resolution_storages)?;

    let jpeg_tables = collect_jpeg_tables(&header, &jpeg_tables_source)?;

    let data_path = best_storage.join("Subimage 0000 Data");
    let data_name = data_path.display().to_string();
    let data_raw = cfbf::read_stream_full(&mut cf, &data_path)?;
    let data_content = cfbf::strip_fpx_prefix(&data_name, &data_raw)?;

    let rgb = tile_decode::decode_resolution(&data_name, data_content, &header, &jpeg_tables)?;

    let (camera_model, capture_date) = read_capture_metadata(&mut cf, &object_root);
    let exif_bytes = exif::build(camera_model.as_deref(), capture_date.as_deref());

    Ok(DecodedImage {
        width: header.width,
        height: header.height,
        rgb,
        exif: exif_bytes,
    })
}

/// Holds the parsed `Image Contents` property set alongside the raw bytes
/// it borrows from, since `PropertySet` borrows its source buffer.
struct ImageContents {
    content: Vec<u8>,
}

impl ImageContents {
    fn property_set(&self) -> Result<PropertySet<'_>> {
        PropertySet::parse("Image Contents", &self.content)
    }
}

fn read_image_contents<F: Read + Seek>(
    cf: &mut cfb::CompoundFile<F>,
    object_root: &Path,
) -> Result<ImageContents> {
    let path = cfbf::find_stream(cf, object_root, "Image Contents")?;
    // Unlike Subimage Header/Data streams, property-set streams have no
    // extra FPX prefix: their 28-byte PROPERTYSETHEADER *is* the stream's
    // first 28 bytes, so we hand the raw bytes straight to PropertySet.
    let content = cfbf::read_stream_full(cf, &path)?;
    Ok(ImageContents { content })
}

/// Reads every `Resolution NNNN` storage's Subimage Header and returns the
/// one with the largest pixel area, per the spec's requirement to never
/// infer resolution rank from the storage name.
fn select_best_resolution<F: Read + Seek>(
    cf: &mut cfb::CompoundFile<F>,
    storages: &[PathBuf],
) -> Result<(PathBuf, SubimageHeader)> {
    let mut best: Option<(PathBuf, SubimageHeader)> = None;
    for storage in storages {
        let header_path = storage.join("Subimage 0000 Header");
        let header_name = header_path.display().to_string();
        let raw = cfbf::read_stream_full(cf, &header_path)?;
        let content = cfbf::strip_fpx_prefix(&header_name, &raw)?;
        let header = subimage_header::parse(&header_name, content)?;

        let area = u64::from(header.width) * u64::from(header.height);
        let is_better = match &best {
            None => true,
            Some((_, current)) => area > u64::from(current.width) * u64::from(current.height),
        };
        if is_better {
            best = Some((storage.clone(), header));
        }
    }
    best.ok_or(FpxError::NoResolutions)
}

/// Fetches every shared JPEG-tables group the chosen resolution's tiles
/// actually reference (group 0 means "self-contained tile", not a lookup).
fn collect_jpeg_tables(
    header: &SubimageHeader,
    image_contents: &ImageContents,
) -> Result<HashMap<u8, Vec<u8>>> {
    let propset = image_contents.property_set()?;

    let mut indices: Vec<u8> = header
        .tiles
        .iter()
        .map(|t| t.jpeg_tables_index())
        .filter(|&i| i != 0)
        .collect();
    indices.sort_unstable();
    indices.dedup();

    let mut tables = HashMap::with_capacity(indices.len());
    for index in indices {
        let blob = propset.get_blob(fpx_ids::pid_jpeg_tables(index), "JPEGTables")?;
        tables.insert(index, blob.to_vec());
    }
    Ok(tables)
}

/// Best-effort capture-date/camera-model lookup: absence or malformed data
/// in `Image Info` doesn't fail the conversion, since none of it is needed
/// to render the image — only to preserve archival context if present.
fn read_capture_metadata<F: Read + Seek>(
    cf: &mut cfb::CompoundFile<F>,
    object_root: &Path,
) -> (Option<String>, Option<String>) {
    let Ok(path) = cfbf::find_stream(cf, object_root, "Image Info") else {
        return (None, None);
    };
    let Ok(content) = cfbf::read_stream_full(cf, &path) else {
        return (None, None);
    };
    let Ok(propset) = PropertySet::parse("Image Info", &content) else {
        return (None, None);
    };

    let model = propset
        .get_lpwstr(fpx_ids::PID_CAMERA_MODEL, "CameraModel")
        .ok();
    let capture_date = propset
        .get_filetime(fpx_ids::PID_CAPTURE_DATE, "CaptureDate")
        .ok()
        .and_then(filetime::format_exif_datetime);

    (model, capture_date)
}
