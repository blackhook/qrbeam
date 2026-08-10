use std::collections::HashSet;

use raptorq::{
    Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation, PayloadId, SourceBlockEncoder,
};

use crate::constants::{SEGMENT_BYTES, SYMBOL_BYTES, SYMBOL_BYTES_U16};
use crate::error::ProtocolError;

const SOURCE_BLOCK_NUMBER: u8 = 0;
const MIN_SEGMENT_BYTES: usize = 1;
const MAX_RAPTORQ_ESI: u32 = 1 << 24;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolPacket {
    pub esi: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct SegmentEncoder {
    index: u32,
    actual_length: usize,
    source_symbol_count: u32,
    encoder: SourceBlockEncoder,
    source_packets: Vec<SymbolPacket>,
}

impl SegmentEncoder {
    /// Builds an independent `RaptorQ` encoder for one file segment.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the segment is empty, exceeds 512 KiB,
    /// or cannot produce the required single source block.
    pub fn new(index: u32, data: &[u8]) -> Result<Self, ProtocolError> {
        validate_segment_length(data.len())?;
        let transfer_length =
            u64::try_from(data.len()).map_err(|_| ProtocolError::SegmentLengthOutOfRange {
                actual: data.len(),
                minimum: MIN_SEGMENT_BYTES,
                maximum: SEGMENT_BYTES,
            })?;
        let config = ObjectTransmissionInformation::new(transfer_length, SYMBOL_BYTES_U16, 1, 1, 1);
        let encoder = Encoder::new(data, config);
        let source_block = encoder
            .get_block_encoders()
            .first()
            .cloned()
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        let source_symbol_count = u32::try_from(data.len().div_ceil(SYMBOL_BYTES))
            .map_err(|_| ProtocolError::InvalidSymbolRange)?;
        let source_packets = source_block
            .source_packets()
            .into_iter()
            .map(packet_from_raptorq)
            .collect();
        Ok(Self {
            index,
            actual_length: data.len(),
            source_symbol_count,
            encoder: source_block,
            source_packets,
        })
    }

    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }

    #[must_use]
    pub const fn actual_length(&self) -> usize {
        self.actual_length
    }

    #[must_use]
    pub const fn source_symbol_count(&self) -> u32 {
        self.source_symbol_count
    }

    #[must_use]
    pub fn source_packets(&self) -> Vec<SymbolPacket> {
        self.source_packets.clone()
    }

    /// Returns a contiguous range of systematic source symbols.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the requested source ESI range is
    /// outside this segment.
    pub fn source_packet_range(
        &self,
        first: u32,
        count: u16,
    ) -> Result<Vec<SymbolPacket>, ProtocolError> {
        let end = first
            .checked_add(u32::from(count))
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        if count == 0 || end > self.source_symbol_count {
            return Err(ProtocolError::InvalidSymbolRange);
        }
        let start_index = usize::try_from(first).map_err(|_| ProtocolError::InvalidSymbolRange)?;
        let end_index = usize::try_from(end).map_err(|_| ProtocolError::InvalidSymbolRange)?;
        Ok(self.source_packets[start_index..end_index].to_vec())
    }

    /// Generates a deterministic range of repair symbols.
    ///
    /// `start` is the zero-based repair symbol offset, not the final ESI. The
    /// returned packet ESI values begin at `source_symbol_count + start`.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the requested range exceeds `RaptorQ`'s
    /// 24-bit ESI space.
    pub fn repair_packets(
        &self,
        start: u32,
        count: u32,
    ) -> Result<Vec<SymbolPacket>, ProtocolError> {
        let end = self
            .source_symbol_count
            .checked_add(start)
            .and_then(|value| value.checked_add(count))
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        if end > MAX_RAPTORQ_ESI {
            return Err(ProtocolError::InvalidSymbolRange);
        }
        Ok(self
            .encoder
            .repair_packets(start, count)
            .into_iter()
            .map(packet_from_raptorq)
            .collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentUpdate {
    Accepted { unique_symbols: u32 },
    Duplicate,
    Complete(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct SegmentDecoder {
    index: u32,
    actual_length: usize,
    expected_crc32c: u32,
    seen_esi: HashSet<u32>,
    decoder: Decoder,
    completed: Option<Vec<u8>>,
}

impl SegmentDecoder {
    /// Builds an independent `RaptorQ` decoder for one file segment.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the expected segment length is empty or
    /// exceeds 512 KiB.
    pub fn new(
        index: u32,
        actual_length: usize,
        expected_crc32c: u32,
    ) -> Result<Self, ProtocolError> {
        validate_segment_length(actual_length)?;
        let transfer_length =
            u64::try_from(actual_length).map_err(|_| ProtocolError::SegmentLengthOutOfRange {
                actual: actual_length,
                minimum: MIN_SEGMENT_BYTES,
                maximum: SEGMENT_BYTES,
            })?;
        let config = ObjectTransmissionInformation::new(transfer_length, SYMBOL_BYTES_U16, 1, 1, 1);
        Ok(Self {
            index,
            actual_length,
            expected_crc32c,
            seen_esi: HashSet::new(),
            decoder: Decoder::new(config),
            completed: None,
        })
    }

    /// Adds one unique `RaptorQ` encoding symbol to this segment.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when ESI or payload length is invalid, or
    /// when a decoded segment fails its expected CRC32C.
    pub fn push(&mut self, packet: SymbolPacket) -> Result<SegmentUpdate, ProtocolError> {
        if packet.data.len() != SYMBOL_BYTES {
            return Err(ProtocolError::InvalidSymbolPayloadLength {
                actual: packet.data.len(),
                expected: SYMBOL_BYTES,
            });
        }
        if packet.esi >= MAX_RAPTORQ_ESI {
            return Err(ProtocolError::InvalidSymbolRange);
        }
        if self.seen_esi.contains(&packet.esi) {
            return Ok(SegmentUpdate::Duplicate);
        }

        let encoded =
            EncodingPacket::new(PayloadId::new(SOURCE_BLOCK_NUMBER, packet.esi), packet.data);
        self.seen_esi.insert(packet.esi);
        if let Some(mut restored) = self.decoder.decode(encoded) {
            restored.truncate(self.actual_length);
            let actual_crc32c = crc32c::crc32c(&restored);
            if actual_crc32c != self.expected_crc32c {
                return Err(ProtocolError::SegmentCrcMismatch {
                    segment_index: self.index,
                    expected: self.expected_crc32c,
                    actual: actual_crc32c,
                });
            }
            self.completed = Some(restored.clone());
            return Ok(SegmentUpdate::Complete(restored));
        }

        let unique_symbols =
            u32::try_from(self.seen_esi.len()).map_err(|_| ProtocolError::InvalidSymbolRange)?;
        Ok(SegmentUpdate::Accepted { unique_symbols })
    }

    #[must_use]
    pub fn completed(&self) -> Option<&[u8]> {
        self.completed.as_deref()
    }

    #[must_use]
    pub fn unique_symbol_count(&self) -> u32 {
        u32::try_from(self.seen_esi.len()).unwrap_or(u32::MAX)
    }
}

fn validate_segment_length(actual: usize) -> Result<(), ProtocolError> {
    if !(MIN_SEGMENT_BYTES..=SEGMENT_BYTES).contains(&actual) {
        return Err(ProtocolError::SegmentLengthOutOfRange {
            actual,
            minimum: MIN_SEGMENT_BYTES,
            maximum: SEGMENT_BYTES,
        });
    }
    Ok(())
}

fn packet_from_raptorq(packet: EncodingPacket) -> SymbolPacket {
    let (payload_id, data) = packet.split();
    SymbolPacket {
        esi: payload_id.encoding_symbol_id(),
        data,
    }
}
