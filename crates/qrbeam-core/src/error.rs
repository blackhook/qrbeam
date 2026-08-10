use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProtocolError {
    #[error("frame is too short: got {actual} bytes, need at least {minimum}")]
    FrameTooShort { actual: usize, minimum: usize },
    #[error("frame magic does not match QRBeam")]
    InvalidMagic,
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u8),
    #[error("unknown frame type {0}")]
    UnknownFrameType(u8),
    #[error("reserved frame byte must be zero, got {0}")]
    ReservedFieldNonZero(u8),
    #[error("invalid frame header length {0}")]
    InvalidHeaderLength(u16),
    #[error("payload is too large: got {actual} bytes, maximum is {maximum}")]
    PayloadTooLarge { actual: usize, maximum: usize },
    #[error("payload length mismatch: header says {declared}, frame contains {actual}")]
    PayloadLengthMismatch { declared: usize, actual: usize },
    #[error("symbol payload length mismatch: expected {expected}, got {actual}")]
    SymbolPayloadLengthMismatch { expected: usize, actual: usize },
    #[error("RaptorQ symbol ID range is invalid")]
    InvalidSymbolRange,
    #[error("frame CRC32C mismatch: expected {expected:#010x}, calculated {actual:#010x}")]
    CrcMismatch { expected: u32, actual: u32 },
    #[error("file is too large: got {actual} bytes, maximum is {maximum}")]
    FileTooLarge { actual: u64, maximum: u64 },
    #[error("manifest is too short: got {actual} bytes, need at least {minimum}")]
    ManifestTooShort { actual: usize, minimum: usize },
    #[error("manifest CRC32C mismatch: expected {expected:#010x}, calculated {actual:#010x}")]
    ManifestCrcMismatch { expected: u32, actual: u32 },
    #[error("manifest BLAKE3 does not match its fragments")]
    ManifestHashMismatch,
    #[error("invalid manifest: {0}")]
    InvalidManifest(&'static str),
    #[error("unknown compression mode {0}")]
    UnknownCompression(u8),
    #[error("unknown QR error correction level {0}")]
    UnknownEccLevel(u8),
    #[error("manifest text field is not valid UTF-8")]
    InvalidManifestUtf8,
    #[error("invalid manifest fragment: {0}")]
    InvalidManifestFragment(&'static str),
    #[error("manifest fragment conflicts with an already received fragment")]
    ManifestFragmentConflict,
    #[error(
        "segment length is outside protocol bounds: got {actual}, expected {minimum}..={maximum}"
    )]
    SegmentLengthOutOfRange {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    #[error("symbol payload length is invalid: got {actual}, expected {expected}")]
    InvalidSymbolPayloadLength { actual: usize, expected: usize },
    #[error(
        "segment {segment_index} CRC32C mismatch: expected {expected:#010x}, calculated {actual:#010x}"
    )]
    SegmentCrcMismatch {
        segment_index: u32,
        expected: u32,
        actual: u32,
    },
    #[error("invalid timeline: {0}")]
    InvalidTimeline(&'static str),
    #[error("timeline does not contain frame {0}")]
    FrameNotAvailable(u64),
    #[error("timeline does not contain segment {0}")]
    SegmentNotAvailable(u32),
    #[error("frame belongs to a different session")]
    SessionMismatch,
    #[error("frame belongs to a different file")]
    FileIdMismatch,
    #[error("frame type cannot be ingested by a data session")]
    UnexpectedFrameType,
    #[error("a validated manifest is required before data frames")]
    ManifestRequired,
    #[error("frame plan is invalid: {0}")]
    InvalidFramePlan(&'static str),
    #[error("restored file length mismatch: expected {expected}, got {actual}")]
    FileLengthMismatch { expected: u64, actual: u64 },
    #[error("restored file BLAKE3 mismatch: expected {expected:?}, calculated {actual:?}")]
    FileHashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}
