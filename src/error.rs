use std::path::PathBuf;

/// Errors surfaced to the CLI. Every variant renders a message that names
/// what went wrong and, where relevant, which stream it came from — per
/// the spec's error-handling requirements (clear error, non-zero exit,
/// no partial output).
#[derive(Debug, thiserror::Error)]
pub enum FpxError {
    #[error("failed to read '{path}': {source}")]
    OpenInput {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("not a FlashPix (.fpx) file: {0}")]
    NotCompoundFile(#[source] std::io::Error),

    #[error("this doesn't look like a FlashPix file: no 'Image Contents' stream found")]
    NotFlashPix,

    #[error("malformed FlashPix file: missing expected stream or storage '{0}'")]
    MissingEntry(String),

    #[error("malformed FlashPix file: could not read '{name}': {source}")]
    StreamRead {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "malformed FlashPix file: '{stream}' is too short (need at least {needed} bytes, found {found})"
    )]
    Truncated {
        stream: String,
        needed: usize,
        found: usize,
    },

    #[error(
        "malformed FlashPix file: property {property:#x} in '{stream}' has the wrong type (expected {expected}, found {found:#x})"
    )]
    WrongPropertyType {
        stream: String,
        property: u32,
        expected: &'static str,
        found: u32,
    },

    #[error(
        "malformed FlashPix file: '{stream}' is missing required property {property:#x} ({name})"
    )]
    MissingProperty {
        stream: String,
        property: u32,
        name: &'static str,
    },

    #[error("malformed FlashPix file: no 'Resolution NNNN' storages found")]
    NoResolutions,

    #[error(
        "unsupported tile compression in '{stream}' (compression type {found}): only JPEG-compressed tiles are supported"
    )]
    UnsupportedCompression { stream: String, found: u32 },

    #[error("failed to decode JPEG data for tile {index} in '{stream}': {source}")]
    TileDecode {
        stream: String,
        index: usize,
        #[source]
        source: jpeg_decoder::Error,
    },

    #[error(
        "tile {index} in '{stream}' decoded to an unexpected pixel format ({found:?}); only 3-channel (RGB) tiles are supported"
    )]
    UnexpectedPixelFormat {
        stream: String,
        index: usize,
        found: jpeg_decoder::PixelFormat,
    },

    #[error("failed to encode PNG output: {0}")]
    PngEncode(#[from] png::EncodingError),

    #[error("failed to write output: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FpxError>;
