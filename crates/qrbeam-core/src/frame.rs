use crate::constants::{
    FRAME_HEADER_BYTES, MAX_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, SYMBOL_BYTES,
};
use crate::error::ProtocolError;

const MAGIC: &[u8; 4] = b"QRBM";
const CRC_OFFSET: usize = 52;
const CRC_END: usize = 56;
const FRAME_HEADER_BYTES_U16: u16 = 56;
const MAX_RAPTORQ_ESI: u32 = 1 << 24;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameType {
    Manifest = 1,
    Test = 2,
    Data = 3,
    Repair = 4,
    Control = 5,
}

impl TryFrom<u8> for FrameType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Manifest),
            2 => Ok(Self::Test),
            3 => Ok(Self::Data),
            4 => Ok(Self::Repair),
            5 => Ok(Self::Control),
            other => Err(ProtocolError::UnknownFrameType(other)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub frame_type: FrameType,
    pub flags: u8,
    pub channel_id: u8,
    pub profile_id: u8,
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub global_frame_index: u64,
    pub segment_index: u32,
    pub first_symbol_id: u32,
    pub symbol_count: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Encodes the frame into the `QRBeam` v1 wire representation.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the payload exceeds QR capacity or a
    /// symbol frame does not contain the declared symbol range.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        validate_payload(&self.header, self.payload.len())?;
        let payload_length =
            u16::try_from(self.payload.len()).map_err(|_| ProtocolError::PayloadTooLarge {
                actual: self.payload.len(),
                maximum: MAX_FRAME_PAYLOAD_BYTES,
            })?;

        let mut bytes = vec![0_u8; FRAME_HEADER_BYTES + self.payload.len()];
        bytes[0..4].copy_from_slice(MAGIC);
        bytes[4] = PROTOCOL_VERSION;
        bytes[5] = self.header.frame_type as u8;
        bytes[6] = self.header.flags;
        bytes[7] = self.header.channel_id;
        bytes[8] = self.header.profile_id;
        bytes[9] = 0;
        bytes[10..12].copy_from_slice(&FRAME_HEADER_BYTES_U16.to_le_bytes());
        bytes[12..28].copy_from_slice(&self.header.session_id);
        bytes[28..32].copy_from_slice(&self.header.file_id.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.header.global_frame_index.to_le_bytes());
        bytes[40..44].copy_from_slice(&self.header.segment_index.to_le_bytes());
        bytes[44..48].copy_from_slice(&self.header.first_symbol_id.to_le_bytes());
        bytes[48..50].copy_from_slice(&self.header.symbol_count.to_le_bytes());
        bytes[50..52].copy_from_slice(&payload_length.to_le_bytes());
        bytes[FRAME_HEADER_BYTES..].copy_from_slice(&self.payload);

        let crc = crc32c::crc32c(&bytes);
        bytes[CRC_OFFSET..CRC_END].copy_from_slice(&crc.to_le_bytes());
        Ok(bytes)
    }

    /// Decodes and validates one complete `QRBeam` v1 frame.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when any header field, payload length,
    /// symbol range, or CRC32C value is invalid.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < FRAME_HEADER_BYTES {
            return Err(ProtocolError::FrameTooShort {
                actual: bytes.len(),
                minimum: FRAME_HEADER_BYTES,
            });
        }
        if &bytes[0..4] != MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }
        if bytes[4] != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(bytes[4]));
        }
        if bytes[9] != 0 {
            return Err(ProtocolError::ReservedFieldNonZero(bytes[9]));
        }

        let header_length = read_u16(bytes, 10);
        if usize::from(header_length) != FRAME_HEADER_BYTES {
            return Err(ProtocolError::InvalidHeaderLength(header_length));
        }

        let payload_length = usize::from(read_u16(bytes, 50));
        if payload_length > MAX_FRAME_PAYLOAD_BYTES {
            return Err(ProtocolError::PayloadTooLarge {
                actual: payload_length,
                maximum: MAX_FRAME_PAYLOAD_BYTES,
            });
        }
        let actual_payload_length = bytes.len() - FRAME_HEADER_BYTES;
        if actual_payload_length != payload_length {
            return Err(ProtocolError::PayloadLengthMismatch {
                declared: payload_length,
                actual: actual_payload_length,
            });
        }

        let expected_crc = read_u32(bytes, CRC_OFFSET);
        let mut actual_crc = crc32c::crc32c(&bytes[..CRC_OFFSET]);
        actual_crc = crc32c::crc32c_append(actual_crc, &[0; 4]);
        actual_crc = crc32c::crc32c_append(actual_crc, &bytes[CRC_END..]);
        if actual_crc != expected_crc {
            return Err(ProtocolError::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }

        let frame_type = FrameType::try_from(bytes[5])?;
        let mut session_id = [0_u8; 16];
        session_id.copy_from_slice(&bytes[12..28]);
        let header = FrameHeader {
            frame_type,
            flags: bytes[6],
            channel_id: bytes[7],
            profile_id: bytes[8],
            session_id,
            file_id: read_u32(bytes, 28),
            global_frame_index: read_u64(bytes, 32),
            segment_index: read_u32(bytes, 40),
            first_symbol_id: read_u32(bytes, 44),
            symbol_count: read_u16(bytes, 48),
        };
        validate_payload(&header, payload_length)?;

        Ok(Self {
            header,
            payload: bytes[FRAME_HEADER_BYTES..].to_vec(),
        })
    }
}

fn validate_payload(header: &FrameHeader, payload_length: usize) -> Result<(), ProtocolError> {
    if payload_length > MAX_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload_length,
            maximum: MAX_FRAME_PAYLOAD_BYTES,
        });
    }

    if matches!(header.frame_type, FrameType::Data | FrameType::Repair) {
        let expected = usize::from(header.symbol_count)
            .checked_mul(SYMBOL_BYTES)
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        if payload_length != expected {
            return Err(ProtocolError::SymbolPayloadLengthMismatch {
                expected,
                actual: payload_length,
            });
        }
        if header.symbol_count == 0 {
            return Err(ProtocolError::InvalidSymbolRange);
        }
        let last_symbol = header
            .first_symbol_id
            .checked_add(u32::from(header.symbol_count) - 1)
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        if last_symbol >= MAX_RAPTORQ_ESI {
            return Err(ProtocolError::InvalidSymbolRange);
        }
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
