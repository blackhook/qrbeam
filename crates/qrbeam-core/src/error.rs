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
}
