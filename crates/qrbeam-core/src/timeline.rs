use std::collections::HashSet;

use crate::constants::MAX_SYMBOLS_PER_FRAME;
use crate::error::ProtocolError;

const MAX_RAPTORQ_ESI: u32 = 1 << 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelRequest {
    pub channel_id: u8,
    pub profile_id: u8,
    pub symbols_per_frame: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Source,
    Repair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FramePlan {
    pub session_id: [u8; 16],
    pub file_id: u32,
    pub channel_id: u8,
    pub profile_id: u8,
    pub global_frame_index: u64,
    pub segment_index: u32,
    pub first_symbol_id: u32,
    pub symbol_count: u16,
    pub kind: SymbolKind,
}

#[derive(Clone, Debug)]
pub struct Timeline {
    session_id: [u8; 16],
    file_id: u32,
    source_symbol_counts: Vec<u32>,
    source_cursor: Vec<u32>,
    repair_cursor: Vec<u32>,
    next_source_segment: usize,
    next_repair_segment: usize,
    repair_only_segment: Option<u32>,
    history: Vec<Vec<FramePlan>>,
    playhead: usize,
}

impl Timeline {
    /// Creates a deterministic timeline for the given file session.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when no segments are present or a segment has
    /// zero source symbols.
    pub fn new(
        session_id: [u8; 16],
        file_id: u32,
        source_symbol_counts: Vec<u32>,
    ) -> Result<Self, ProtocolError> {
        if source_symbol_counts.is_empty() {
            return Err(ProtocolError::InvalidTimeline("no segments"));
        }
        if source_symbol_counts.contains(&0) {
            return Err(ProtocolError::InvalidTimeline(
                "segment has zero source symbols",
            ));
        }
        let segment_count = source_symbol_counts.len();
        Ok(Self {
            session_id,
            file_id,
            source_symbol_counts,
            source_cursor: vec![0; segment_count],
            repair_cursor: vec![0; segment_count],
            next_source_segment: 0,
            next_repair_segment: 0,
            repair_only_segment: None,
            history: Vec::new(),
            playhead: 0,
        })
    }

    /// Replays one historical tick or appends one new deterministic tick.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when channels are empty, duplicated, request
    /// invalid symbol counts, or exhaust the 24-bit ESI range.
    pub fn next_tick(
        &mut self,
        channels: &[ChannelRequest],
    ) -> Result<Vec<FramePlan>, ProtocolError> {
        validate_channels(channels)?;
        if self.playhead < self.history.len() {
            let result = self.history[self.playhead].clone();
            self.playhead += 1;
            return Ok(result);
        }

        let global_frame_index = u64::try_from(self.history.len())
            .map_err(|_| ProtocolError::InvalidTimeline("frame index overflow"))?;
        let mut plans = Vec::with_capacity(channels.len());
        for channel in channels {
            let plan = if let Some(segment_index) = self.repair_only_segment {
                self.allocate_repair(segment_index, *channel, global_frame_index)?
            } else if self.has_source_symbols() {
                self.allocate_source(*channel, global_frame_index)?
            } else {
                let segment_index = u32::try_from(self.next_repair_segment)
                    .map_err(|_| ProtocolError::InvalidTimeline("segment index overflow"))?;
                let plan = self.allocate_repair(segment_index, *channel, global_frame_index)?;
                self.next_repair_segment =
                    (self.next_repair_segment + 1) % self.source_symbol_counts.len();
                plan
            };
            plans.push(plan);
        }
        self.history.push(plans.clone());
        self.playhead = self.history.len();
        Ok(plans)
    }

    /// Moves the playhead to an existing frame or the current live edge.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the requested index is beyond the live
    /// edge of the generated history.
    pub fn seek_frame(&mut self, global_frame_index: u64) -> Result<u64, ProtocolError> {
        let requested = usize::try_from(global_frame_index)
            .map_err(|_| ProtocolError::FrameNotAvailable(global_frame_index))?;
        if requested > self.history.len() {
            return Err(ProtocolError::FrameNotAvailable(global_frame_index));
        }
        self.playhead = requested;
        Ok(global_frame_index)
    }

    pub fn seek_back(&mut self, frames: usize) -> u64 {
        self.playhead = self.playhead.saturating_sub(frames);
        u64::try_from(self.playhead).unwrap_or(u64::MAX)
    }

    pub fn seek_forward(&mut self, frames: usize) -> u64 {
        self.playhead = self.playhead.saturating_add(frames).min(self.history.len());
        u64::try_from(self.playhead).unwrap_or(u64::MAX)
    }

    /// Moves the playhead to the most recent historical tick for a segment.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the segment has no generated history.
    pub fn seek_segment(&mut self, segment_index: u32) -> Result<u64, ProtocolError> {
        let position = self
            .history
            .iter()
            .rposition(|tick| tick.iter().any(|plan| plan.segment_index == segment_index))
            .ok_or(ProtocolError::SegmentNotAvailable(segment_index))?;
        self.playhead = position;
        u64::try_from(position).map_err(|_| ProtocolError::InvalidTimeline("frame index overflow"))
    }

    /// Starts appending repair-only ticks for one segment at the live edge.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the segment index is outside this file.
    pub fn start_repair(&mut self, segment_index: u32) -> Result<(), ProtocolError> {
        let index = usize::try_from(segment_index)
            .map_err(|_| ProtocolError::SegmentNotAvailable(segment_index))?;
        if index >= self.source_symbol_counts.len() {
            return Err(ProtocolError::SegmentNotAvailable(segment_index));
        }
        self.repair_only_segment = Some(segment_index);
        self.playhead = self.history.len();
        Ok(())
    }

    pub fn stop_repair(&mut self) {
        self.repair_only_segment = None;
        self.playhead = self.history.len();
    }

    #[must_use]
    pub fn current_frame(&self) -> u64 {
        u64::try_from(self.playhead).unwrap_or(u64::MAX)
    }

    fn has_source_symbols(&mut self) -> bool {
        while self.next_source_segment < self.source_symbol_counts.len()
            && self.source_cursor[self.next_source_segment]
                >= self.source_symbol_counts[self.next_source_segment]
        {
            self.next_source_segment += 1;
        }
        self.next_source_segment < self.source_symbol_counts.len()
    }

    fn allocate_source(
        &mut self,
        channel: ChannelRequest,
        global_frame_index: u64,
    ) -> Result<FramePlan, ProtocolError> {
        if !self.has_source_symbols() {
            return Err(ProtocolError::InvalidTimeline(
                "source allocation requested after source phase",
            ));
        }
        let index = self.next_source_segment;
        let first_symbol_id = self.source_cursor[index];
        let remaining = self.source_symbol_counts[index] - first_symbol_id;
        let symbol_count = u16::try_from(remaining.min(u32::from(channel.symbols_per_frame)))
            .map_err(|_| ProtocolError::InvalidTimeline("symbol count overflow"))?;
        self.source_cursor[index] += u32::from(symbol_count);
        let segment_index = u32::try_from(index)
            .map_err(|_| ProtocolError::InvalidTimeline("segment index overflow"))?;
        Ok(self.plan(
            channel,
            global_frame_index,
            segment_index,
            first_symbol_id,
            symbol_count,
            SymbolKind::Source,
        ))
    }

    fn allocate_repair(
        &mut self,
        segment_index: u32,
        channel: ChannelRequest,
        global_frame_index: u64,
    ) -> Result<FramePlan, ProtocolError> {
        let index = usize::try_from(segment_index)
            .map_err(|_| ProtocolError::SegmentNotAvailable(segment_index))?;
        if index >= self.source_symbol_counts.len() {
            return Err(ProtocolError::SegmentNotAvailable(segment_index));
        }
        let first_symbol_id = self.source_symbol_counts[index]
            .checked_add(self.repair_cursor[index])
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        let end = first_symbol_id
            .checked_add(u32::from(channel.symbols_per_frame))
            .ok_or(ProtocolError::InvalidSymbolRange)?;
        if end > MAX_RAPTORQ_ESI {
            return Err(ProtocolError::InvalidSymbolRange);
        }
        self.repair_cursor[index] += u32::from(channel.symbols_per_frame);
        Ok(self.plan(
            channel,
            global_frame_index,
            segment_index,
            first_symbol_id,
            channel.symbols_per_frame,
            SymbolKind::Repair,
        ))
    }

    const fn plan(
        &self,
        channel: ChannelRequest,
        global_frame_index: u64,
        segment_index: u32,
        first_symbol_id: u32,
        symbol_count: u16,
        kind: SymbolKind,
    ) -> FramePlan {
        FramePlan {
            session_id: self.session_id,
            file_id: self.file_id,
            channel_id: channel.channel_id,
            profile_id: channel.profile_id,
            global_frame_index,
            segment_index,
            first_symbol_id,
            symbol_count,
            kind,
        }
    }
}

fn validate_channels(channels: &[ChannelRequest]) -> Result<(), ProtocolError> {
    if channels.is_empty() {
        return Err(ProtocolError::InvalidTimeline("no channels"));
    }
    let mut channel_ids = HashSet::with_capacity(channels.len());
    for channel in channels {
        if channel.symbols_per_frame == 0 || channel.symbols_per_frame > MAX_SYMBOLS_PER_FRAME {
            return Err(ProtocolError::InvalidTimeline(
                "channel symbol count is outside protocol bounds",
            ));
        }
        if !channel_ids.insert(channel.channel_id) {
            return Err(ProtocolError::InvalidTimeline("duplicate channel ID"));
        }
    }
    Ok(())
}
