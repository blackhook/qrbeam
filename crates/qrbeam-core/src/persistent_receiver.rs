use std::collections::HashSet;

use crate::constants::{SEGMENT_BYTES, SYMBOL_BYTES};
use crate::error::ProtocolError;
use crate::frame::{Frame, FrameType};
use crate::manifest::Manifest;
use crate::receiver::{ReceiverPhase, ReceiverSnapshot};
use crate::segment::{SegmentDecoder, SegmentUpdate, SymbolPacket};
use crate::session::BlockState;

const MAX_ACTIVE_DECODERS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentUpdate {
    Accepted,
    IgnoredDuplicate,
    SegmentReady { index: u32, bytes: Vec<u8> },
    ManifestRefreshed,
}

#[derive(Clone, Debug)]
struct ActiveDecoder {
    decoder: SegmentDecoder,
    last_used: u64,
}

#[derive(Debug)]
pub struct PersistentReceiver {
    manifest: Manifest,
    active: Vec<Option<ActiveDecoder>>,
    completed: Vec<bool>,
    pending: Vec<Option<Vec<u8>>>,
    unique: Vec<u32>,
    required: Vec<u32>,
    failed: Vec<bool>,
    seen_frames: HashSet<(u64, u8)>,
    snapshot: ReceiverSnapshot,
    clock: u64,
}

impl PersistentReceiver {
    /// Creates a storage-agnostic receiver for an already validated manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the manifest cannot be encoded or its
    /// segment metadata is outside the v1 protocol bounds.
    pub fn from_manifest(manifest: Manifest) -> Result<Self, ProtocolError> {
        manifest.encode()?;
        let count = manifest.segment_crc32c.len();
        let mut required = Vec::with_capacity(count);
        for index in 0..count {
            let length = segment_length(&manifest, index)?;
            required.push(
                u32::try_from(length.div_ceil(SYMBOL_BYTES))
                    .map_err(|_| ProtocolError::InvalidSymbolRange)?,
            );
        }
        Ok(Self {
            snapshot: ReceiverSnapshot {
                phase: ReceiverPhase::Receiving,
                filename: Some(manifest.filename.clone()),
                total_bytes: Some(manifest.original_length),
                blocks: vec![BlockState::Missing; count],
                last_frame_index: None,
            },
            manifest,
            active: vec![None; count],
            completed: vec![false; count],
            pending: vec![None; count],
            unique: vec![0; count],
            required,
            failed: vec![false; count],
            seen_frames: HashSet::new(),
            clock: 0,
        })
    }

    /// Ingests one data or repair frame. A decoded segment remains partial
    /// until its caller persists the bytes and acknowledges it.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the frame is invalid or foreign.
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<PersistentUpdate, ProtocolError> {
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
        let identity = (frame.header.global_frame_index, frame.header.channel_id);
        if !self.seen_frames.insert(identity) {
            return Ok(PersistentUpdate::IgnoredDuplicate);
        }
        let index = usize::try_from(frame.header.segment_index)
            .map_err(|_| ProtocolError::SegmentNotAvailable(frame.header.segment_index))?;
        if index >= self.completed.len() {
            return Err(ProtocolError::SegmentNotAvailable(
                frame.header.segment_index,
            ));
        }
        self.snapshot.last_frame_index = Some(frame.header.global_frame_index);
        if self.completed[index] || self.pending[index].is_some() {
            return Ok(PersistentUpdate::IgnoredDuplicate);
        }
        validate_frame_range(&frame, self.required[index])?;
        self.clock = self.clock.wrapping_add(1);
        self.ensure_decoder(index)?;
        let active = self.active[index]
            .as_mut()
            .expect("decoder was initialized");
        active.last_used = self.clock;
        let mut accepted = false;
        let mut ready = None;
        for (offset, symbol) in frame.payload.chunks_exact(SYMBOL_BYTES).enumerate() {
            let esi = frame
                .header
                .first_symbol_id
                .checked_add(
                    u32::try_from(offset)
                        .map_err(|_| ProtocolError::InvalidFramePlan("packet offset overflow"))?,
                )
                .ok_or(ProtocolError::InvalidSymbolRange)?;
            match active.decoder.push(SymbolPacket {
                esi,
                data: symbol.to_vec(),
            }) {
                Ok(SegmentUpdate::Accepted { unique_symbols }) => {
                    self.unique[index] = unique_symbols;
                    accepted = true;
                }
                Ok(SegmentUpdate::Duplicate) => {}
                Ok(SegmentUpdate::Complete(data)) => {
                    self.unique[index] = self.required[index];
                    self.failed[index] = false;
                    ready = Some(data);
                    accepted = true;
                }
                Err(error @ ProtocolError::SegmentCrcMismatch { .. }) => {
                    self.unique[index] = active.decoder.unique_symbol_count();
                    self.failed[index] = true;
                    self.refresh_blocks();
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
        if let Some(data) = ready {
            self.pending[index] = Some(data.clone());
            self.refresh_blocks();
            return Ok(PersistentUpdate::SegmentReady {
                index: frame.header.segment_index,
                bytes: data,
            });
        }
        self.refresh_blocks();
        Ok(if accepted {
            PersistentUpdate::Accepted
        } else {
            PersistentUpdate::IgnoredDuplicate
        })
    }

    /// Marks one previously emitted segment as durably stored.
    pub fn acknowledge_segment(&mut self, index: u32) -> Result<(), ProtocolError> {
        let index =
            usize::try_from(index).map_err(|_| ProtocolError::SegmentNotAvailable(index))?;
        if self.pending.get(index).and_then(Option::as_ref).is_none() {
            return Err(ProtocolError::SegmentNotAvailable(
                u32::try_from(index).unwrap_or(u32::MAX),
            ));
        }
        self.pending[index] = None;
        self.active[index] = None;
        self.completed[index] = true;
        self.refresh_blocks();
        Ok(())
    }

    /// Restores a durably stored segment without retaining its bytes in RAM.
    pub fn restore_completed_segment(&mut self, index: u32) -> Result<(), ProtocolError> {
        let index =
            usize::try_from(index).map_err(|_| ProtocolError::SegmentNotAvailable(index))?;
        if index >= self.completed.len() {
            return Err(ProtocolError::SegmentNotAvailable(
                u32::try_from(index).unwrap_or(u32::MAX),
            ));
        }
        self.pending[index] = None;
        self.active[index] = None;
        self.completed[index] = true;
        self.refresh_blocks();
        Ok(())
    }

    /// Replays persisted valid frames to rebuild one partial decoder.
    pub fn restore_partial_frames(
        &mut self,
        index: u32,
        frames: &[Vec<u8>],
    ) -> Result<(), ProtocolError> {
        for frame in frames {
            let decoded = Frame::decode(frame)?;
            if decoded.header.segment_index == index {
                self.seen_frames
                    .remove(&(decoded.header.global_frame_index, decoded.header.channel_id));
                let _ = self.ingest(frame)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn snapshot(&self) -> &ReceiverSnapshot {
        &self.snapshot
    }

    fn ensure_decoder(&mut self, index: usize) -> Result<(), ProtocolError> {
        if self.active[index].is_some() {
            return Ok(());
        }
        let live = self.active.iter().flatten().count();
        if live >= MAX_ACTIVE_DECODERS {
            if let Some((oldest, _)) = self
                .active
                .iter()
                .enumerate()
                .filter_map(|(i, value)| value.as_ref().map(|active| (i, active.last_used)))
                .min_by_key(|(_, used)| *used)
            {
                self.active[oldest] = None;
            }
        }
        let length = segment_length(&self.manifest, index)?;
        let checksum = self.manifest.segment_crc32c[index];
        self.active[index] = Some(ActiveDecoder {
            decoder: SegmentDecoder::new(
                u32::try_from(index)
                    .map_err(|_| ProtocolError::InvalidManifest("too many segments"))?,
                length,
                checksum,
            )?,
            last_used: self.clock,
        });
        Ok(())
    }

    fn refresh_blocks(&mut self) {
        self.snapshot.blocks = self
            .completed
            .iter()
            .enumerate()
            .map(|(index, done)| {
                if *done {
                    BlockState::Complete
                } else if self.failed[index] {
                    BlockState::Failed {
                        unique: self.unique[index],
                        required: self.required[index],
                    }
                } else if self.unique[index] == 0 {
                    BlockState::Missing
                } else {
                    BlockState::Partial {
                        unique: self.unique[index],
                        required: self.required[index],
                    }
                }
            })
            .collect();
    }
}

fn segment_length(manifest: &Manifest, index: usize) -> Result<usize, ProtocolError> {
    if index + 1 == manifest.segment_crc32c.len() {
        usize::try_from(manifest.last_segment_length)
            .map_err(|_| ProtocolError::InvalidManifest("last segment is too large"))
    } else {
        Ok(SEGMENT_BYTES)
    }
}

fn validate_frame_range(frame: &Frame, required: u32) -> Result<(), ProtocolError> {
    let end = frame
        .header
        .first_symbol_id
        .checked_add(u32::from(frame.header.symbol_count))
        .ok_or(ProtocolError::InvalidSymbolRange)?;
    match frame.header.frame_type {
        FrameType::Data if end > required => Err(ProtocolError::InvalidFramePlan(
            "data frame crosses the source symbol boundary",
        )),
        FrameType::Repair if frame.header.first_symbol_id < required => Err(
            ProtocolError::InvalidFramePlan("repair frame starts inside source symbols"),
        ),
        FrameType::Data | FrameType::Repair => Ok(()),
        _ => Err(ProtocolError::UnexpectedFrameType),
    }
}
