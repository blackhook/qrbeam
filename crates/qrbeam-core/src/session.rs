use std::collections::HashSet;

use crate::constants::{
    MAX_FILE_BYTES, SEGMENT_BYTES, SEGMENT_BYTES_U32, SYMBOL_BYTES, SYMBOL_BYTES_U16,
};
use crate::error::ProtocolError;
use crate::frame::{Frame, FrameHeader, FrameType};
use crate::manifest::{Compression, Manifest, Profile};
use crate::segment::{SegmentDecoder, SegmentEncoder, SegmentUpdate, SymbolPacket};
use crate::timeline::{ChannelRequest, FramePlan, SymbolKind, Timeline};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockState {
    Missing,
    Partial { unique: u32, required: u32 },
    Complete,
    Failed { unique: u32, required: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiveUpdate {
    IgnoredDuplicate,
    Accepted,
    BlockComplete(u32),
    Complete(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct SendSession {
    manifest: Manifest,
    segment_encoders: Vec<SegmentEncoder>,
    timeline: Timeline,
}

impl SendSession {
    /// Creates an in-memory sender session for the protocol-core milestone.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] for empty or oversized files, invalid segment
    /// metadata, or an invalid generated timeline.
    pub fn new(
        filename: &str,
        mime_type: &str,
        data: &[u8],
        session_id: [u8; 16],
        file_id: u32,
    ) -> Result<Self, ProtocolError> {
        if data.is_empty() {
            return Err(ProtocolError::InvalidManifest(
                "empty files use the manifest-only transfer path",
            ));
        }
        let data_length = u64::try_from(data.len()).map_err(|_| ProtocolError::FileTooLarge {
            actual: u64::MAX,
            maximum: MAX_FILE_BYTES,
        })?;
        if data_length > MAX_FILE_BYTES {
            return Err(ProtocolError::FileTooLarge {
                actual: data_length,
                maximum: MAX_FILE_BYTES,
            });
        }

        let mut segment_encoders = Vec::with_capacity(data.len().div_ceil(SEGMENT_BYTES));
        let mut segment_crc32c = Vec::with_capacity(segment_encoders.capacity());
        for (index, segment) in data.chunks(SEGMENT_BYTES).enumerate() {
            let segment_index = u32::try_from(index)
                .map_err(|_| ProtocolError::InvalidManifest("too many segments"))?;
            segment_crc32c.push(crc32c::crc32c(segment));
            segment_encoders.push(SegmentEncoder::new(segment_index, segment)?);
        }
        let source_symbol_counts = segment_encoders
            .iter()
            .map(SegmentEncoder::source_symbol_count)
            .collect();
        let timeline = Timeline::new(session_id, file_id, source_symbol_counts)?;

        let mut seed_hasher = blake3::Hasher::new();
        seed_hasher.update(&session_id);
        seed_hasher.update(&file_id.to_le_bytes());
        let mut encoding_seed = [0_u8; 16];
        encoding_seed.copy_from_slice(&seed_hasher.finalize().as_bytes()[..16]);
        let last_segment_length = u32::try_from(
            data.len()
                .checked_sub((segment_encoders.len() - 1) * SEGMENT_BYTES)
                .ok_or(ProtocolError::InvalidManifest("last segment underflow"))?,
        )
        .map_err(|_| ProtocolError::InvalidManifest("last segment is too large"))?;
        let manifest = Manifest {
            session_id,
            file_id,
            filename: sanitize_filename(filename),
            mime_type: mime_type.to_owned(),
            original_length: data_length,
            container_length: data_length,
            compression: Compression::None,
            file_hash: *blake3::hash(data).as_bytes(),
            segment_size: SEGMENT_BYTES_U32,
            symbol_size: SYMBOL_BYTES_U16,
            last_segment_length,
            encoding_seed,
            profiles: Profile::defaults().to_vec(),
            segment_crc32c,
        };
        manifest.encode()?;
        Ok(Self {
            manifest,
            segment_encoders,
            timeline,
        })
    }

    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub const fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    /// Generates and encodes the next display tick for all requested channels.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when a channel does not match its manifest
    /// profile, the timeline cannot allocate symbols, or frame encoding fails.
    pub fn next_frames(
        &mut self,
        channels: &[ChannelRequest],
    ) -> Result<Vec<Vec<u8>>, ProtocolError> {
        for channel in channels {
            let profile = self
                .manifest
                .profiles
                .iter()
                .find(|profile| profile.id == channel.profile_id)
                .ok_or(ProtocolError::InvalidTimeline("unknown profile ID"))?;
            if u16::from(profile.symbols_per_frame) != channel.symbols_per_frame {
                return Err(ProtocolError::InvalidTimeline(
                    "profile symbol count does not match manifest",
                ));
            }
        }
        let plans = self.timeline.next_tick(channels)?;
        plans
            .iter()
            .map(|plan| self.frame_for_plan(plan)?.encode())
            .collect()
    }

    /// Reconstructs one deterministic frame from its timeline metadata.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when identity, segment, or symbol range does
    /// not belong to this sender session.
    pub fn frame_for_plan(&self, plan: &FramePlan) -> Result<Frame, ProtocolError> {
        if plan.session_id != self.manifest.session_id {
            return Err(ProtocolError::SessionMismatch);
        }
        if plan.file_id != self.manifest.file_id {
            return Err(ProtocolError::FileIdMismatch);
        }
        let segment_index = usize::try_from(plan.segment_index)
            .map_err(|_| ProtocolError::SegmentNotAvailable(plan.segment_index))?;
        let encoder = self
            .segment_encoders
            .get(segment_index)
            .ok_or(ProtocolError::SegmentNotAvailable(plan.segment_index))?;
        let packets = match plan.kind {
            SymbolKind::Source => {
                encoder.source_packet_range(plan.first_symbol_id, plan.symbol_count)?
            }
            SymbolKind::Repair => {
                let repair_offset = plan
                    .first_symbol_id
                    .checked_sub(encoder.source_symbol_count())
                    .ok_or(ProtocolError::InvalidFramePlan(
                        "repair ESI precedes source symbols",
                    ))?;
                encoder.repair_packets(repair_offset, u32::from(plan.symbol_count))?
            }
        };
        let mut payload = Vec::with_capacity(usize::from(plan.symbol_count) * SYMBOL_BYTES);
        for (offset, packet) in packets.into_iter().enumerate() {
            let expected_esi = plan
                .first_symbol_id
                .checked_add(
                    u32::try_from(offset)
                        .map_err(|_| ProtocolError::InvalidFramePlan("packet offset overflow"))?,
                )
                .ok_or(ProtocolError::InvalidSymbolRange)?;
            if packet.esi != expected_esi {
                return Err(ProtocolError::InvalidFramePlan(
                    "encoder returned a non-contiguous ESI",
                ));
            }
            payload.extend_from_slice(&packet.data);
        }
        Ok(Frame {
            header: FrameHeader {
                frame_type: match plan.kind {
                    SymbolKind::Source => FrameType::Data,
                    SymbolKind::Repair => FrameType::Repair,
                },
                flags: 0,
                channel_id: plan.channel_id,
                profile_id: plan.profile_id,
                session_id: plan.session_id,
                file_id: plan.file_id,
                global_frame_index: plan.global_frame_index,
                segment_index: plan.segment_index,
                first_symbol_id: plan.first_symbol_id,
                symbol_count: plan.symbol_count,
            },
            payload,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReceiveSession {
    manifest: Manifest,
    segment_decoders: Vec<SegmentDecoder>,
    completed_segments: Vec<Option<Vec<u8>>>,
    unique_symbols: Vec<u32>,
    required_symbols: Vec<u32>,
    failed_segments: Vec<bool>,
    seen_frames: HashSet<(u64, u8)>,
    completed_file: Option<Vec<u8>>,
}

impl ReceiveSession {
    /// Creates a receiver after a complete manifest has been validated.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when manifest or derived segment metadata is
    /// invalid.
    pub fn new(manifest: Manifest) -> Result<Self, ProtocolError> {
        manifest.encode()?;
        let mut segment_decoders = Vec::with_capacity(manifest.segment_crc32c.len());
        let mut required_symbols = Vec::with_capacity(manifest.segment_crc32c.len());
        for (index, checksum) in manifest.segment_crc32c.iter().copied().enumerate() {
            let actual_length = if index + 1 == manifest.segment_crc32c.len() {
                usize::try_from(manifest.last_segment_length)
                    .map_err(|_| ProtocolError::InvalidManifest("last segment is too large"))?
            } else {
                SEGMENT_BYTES
            };
            let segment_index = u32::try_from(index)
                .map_err(|_| ProtocolError::InvalidManifest("too many segments"))?;
            segment_decoders.push(SegmentDecoder::new(segment_index, actual_length, checksum)?);
            required_symbols.push(
                u32::try_from(actual_length.div_ceil(SYMBOL_BYTES))
                    .map_err(|_| ProtocolError::InvalidSymbolRange)?,
            );
        }
        let segment_count = segment_decoders.len();
        Ok(Self {
            manifest,
            segment_decoders,
            completed_segments: vec![None; segment_count],
            unique_symbols: vec![0; segment_count],
            required_symbols,
            failed_segments: vec![false; segment_count],
            seen_frames: HashSet::new(),
            completed_file: None,
        })
    }

    /// Validates and ingests one encoded data or repair frame.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] for invalid frame bytes, foreign identities,
    /// symbol metadata, segment CRC, final length, or final BLAKE3.
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<ReceiveUpdate, ProtocolError> {
        let frame = Frame::decode(bytes)?;
        if frame.header.session_id != self.manifest.session_id {
            return Err(ProtocolError::SessionMismatch);
        }
        if frame.header.file_id != self.manifest.file_id {
            return Err(ProtocolError::FileIdMismatch);
        }
        if !matches!(frame.header.frame_type, FrameType::Data | FrameType::Repair) {
            return Err(ProtocolError::UnexpectedFrameType);
        }
        let frame_identity = (frame.header.global_frame_index, frame.header.channel_id);
        if self.seen_frames.contains(&frame_identity) {
            return Ok(ReceiveUpdate::IgnoredDuplicate);
        }
        let segment_index = usize::try_from(frame.header.segment_index)
            .map_err(|_| ProtocolError::SegmentNotAvailable(frame.header.segment_index))?;
        if segment_index >= self.segment_decoders.len() {
            return Err(ProtocolError::SegmentNotAvailable(
                frame.header.segment_index,
            ));
        }
        if self.completed_segments[segment_index].is_some() {
            self.seen_frames.insert(frame_identity);
            return Ok(ReceiveUpdate::IgnoredDuplicate);
        }
        let required = self.required_symbols[segment_index];
        let end_esi = frame
            .header
            .first_symbol_id
            .checked_add(u32::from(frame.header.symbol_count))
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        match frame.header.frame_type {
            FrameType::Data if end_esi > required => {
                return Err(ProtocolError::InvalidFramePlan(
                    "data frame crosses the source symbol boundary",
                ));
            }
            FrameType::Repair if frame.header.first_symbol_id < required => {
                return Err(ProtocolError::InvalidFramePlan(
                    "repair frame starts inside source symbols",
                ));
            }
            FrameType::Data | FrameType::Repair => {}
            _ => return Err(ProtocolError::UnexpectedFrameType),
        }

        self.seen_frames.insert(frame_identity);
        let mut accepted = false;
        let mut completed = None;
        for (offset, symbol) in frame.payload.chunks_exact(SYMBOL_BYTES).enumerate() {
            let esi = frame
                .header
                .first_symbol_id
                .checked_add(
                    u32::try_from(offset)
                        .map_err(|_| ProtocolError::InvalidFramePlan("packet offset overflow"))?,
                )
                .ok_or(ProtocolError::InvalidSymbolRange)?;
            let update = self.segment_decoders[segment_index].push(SymbolPacket {
                esi,
                data: symbol.to_vec(),
            });
            let update = match update {
                Ok(update) => update,
                Err(error @ ProtocolError::SegmentCrcMismatch { .. }) => {
                    self.unique_symbols[segment_index] =
                        self.segment_decoders[segment_index].unique_symbol_count();
                    self.failed_segments[segment_index] = true;
                    return Err(error);
                }
                Err(error) => return Err(error),
            };
            match update {
                SegmentUpdate::Accepted { unique_symbols } => {
                    self.unique_symbols[segment_index] = unique_symbols;
                    accepted = true;
                }
                SegmentUpdate::Duplicate => {}
                SegmentUpdate::Complete(data) => {
                    self.unique_symbols[segment_index] = self.required_symbols[segment_index];
                    self.failed_segments[segment_index] = false;
                    completed = Some(data);
                    accepted = true;
                }
            }
        }
        if let Some(data) = completed {
            self.completed_segments[segment_index] = Some(data);
            if self.completed_segments.iter().all(Option::is_some) {
                let restored = self.assemble_file()?;
                self.completed_file = Some(restored.clone());
                return Ok(ReceiveUpdate::Complete(restored));
            }
            return Ok(ReceiveUpdate::BlockComplete(frame.header.segment_index));
        }
        if accepted {
            Ok(ReceiveUpdate::Accepted)
        } else {
            Ok(ReceiveUpdate::IgnoredDuplicate)
        }
    }

    #[must_use]
    pub fn block_states(&self) -> Vec<BlockState> {
        self.completed_segments
            .iter()
            .enumerate()
            .map(|(index, completed)| {
                if completed.is_some() {
                    BlockState::Complete
                } else if self.failed_segments[index] {
                    BlockState::Failed {
                        unique: self.unique_symbols[index],
                        required: self.required_symbols[index],
                    }
                } else if self.unique_symbols[index] == 0 {
                    BlockState::Missing
                } else {
                    BlockState::Partial {
                        unique: self.unique_symbols[index],
                        required: self.required_symbols[index],
                    }
                }
            })
            .collect()
    }

    #[must_use]
    pub fn completed_file(&self) -> Option<&[u8]> {
        self.completed_file.as_deref()
    }

    fn assemble_file(&self) -> Result<Vec<u8>, ProtocolError> {
        let expected_length = usize::try_from(self.manifest.container_length).map_err(|_| {
            ProtocolError::FileLengthMismatch {
                expected: self.manifest.container_length,
                actual: u64::MAX,
            }
        })?;
        let mut restored = Vec::with_capacity(expected_length);
        for segment in self.completed_segments.iter().flatten() {
            restored.extend_from_slice(segment);
        }
        let actual_length = u64::try_from(restored.len()).unwrap_or(u64::MAX);
        if actual_length != self.manifest.container_length {
            return Err(ProtocolError::FileLengthMismatch {
                expected: self.manifest.container_length,
                actual: actual_length,
            });
        }
        let actual_hash = *blake3::hash(&restored).as_bytes();
        if actual_hash != self.manifest.file_hash {
            return Err(ProtocolError::FileHashMismatch {
                expected: self.manifest.file_hash,
                actual: actual_hash,
            });
        }
        Ok(restored)
    }
}

fn sanitize_filename(filename: &str) -> String {
    let sanitized: String = filename
        .chars()
        .map(|character| {
            if character == '/' || character == '\\' || character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();
    let trimmed = sanitized.trim().trim_matches('.');
    if trimmed.is_empty() {
        "received.bin".to_owned()
    } else {
        trimmed.to_owned()
    }
}
