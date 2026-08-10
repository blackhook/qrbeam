use qrbeam_core::error::ProtocolError;
use qrbeam_core::manifest::EccLevel;
use qrbeam_core::manifest_carousel::ManifestCarousel;
use qrbeam_core::session::SendSession;
use qrbeam_core::timeline::ChannelRequest;
use thiserror::Error;

const MANIFEST_SECONDS: u32 = 3;
const MANIFEST_INTERVAL_SECONDS: u32 = 2;
const STABLE_PROFILE_ID: u8 = 0;
const STABLE_CHANNEL_ID: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackPhase {
    Manifest,
    Data,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisplayFrame {
    pub bytes: Vec<u8>,
    pub phase: PlaybackPhase,
    pub logical_index: u64,
    pub ecc: EccLevel,
}

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("FPS must be between 1 and 8, got {0}")]
    InvalidFps(u8),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

#[derive(Clone, Debug)]
pub struct Player {
    sender: SendSession,
    manifest_carousel: ManifestCarousel,
    manifest_ticks: u32,
    manifest_ticks_emitted: u32,
    manifest_round_remaining: usize,
    ticks_since_manifest_round: u32,
    manifest_interval_ticks: u32,
    paused: bool,
    last_frame: Option<DisplayFrame>,
}

impl Player {
    /// Creates a single-channel stable-profile player.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] when FPS is outside 1..=8 or the manifest
    /// cannot be fragmented into valid wire frames.
    pub fn new(sender: SendSession, fps: u8) -> Result<Self, PlayerError> {
        if !(1..=8).contains(&fps) {
            return Err(PlayerError::InvalidFps(fps));
        }
        let session_id = sender.manifest().session_id;
        let file_id = sender.manifest().file_id;
        let manifest_frames = sender
            .manifest()
            .fragments()?
            .into_iter()
            .enumerate()
            .map(|(index, fragment)| {
                let global_frame_index = u64::try_from(index)
                    .map_err(|_| ProtocolError::InvalidTimeline("manifest index overflow"))?;
                fragment
                    .into_frame(
                        session_id,
                        file_id,
                        global_frame_index,
                        STABLE_CHANNEL_ID,
                        STABLE_PROFILE_ID,
                    )?
                    .encode()
            })
            .collect::<Result<Vec<_>, ProtocolError>>()?;
        let manifest_carousel = ManifestCarousel::new(manifest_frames)?;
        Ok(Self {
            sender,
            manifest_carousel,
            manifest_ticks: u32::from(fps) * MANIFEST_SECONDS,
            manifest_ticks_emitted: 0,
            manifest_round_remaining: 0,
            ticks_since_manifest_round: 0,
            manifest_interval_ticks: u32::from(fps) * MANIFEST_INTERVAL_SECONDS,
            paused: false,
            last_frame: None,
        })
    }

    /// Produces the next display frame or repeats the current one while paused.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] when the deterministic send timeline cannot
    /// allocate or encode the next stable-channel frame.
    pub fn next_frame(&mut self) -> Result<DisplayFrame, PlayerError> {
        if self.paused
            && let Some(frame) = self.last_frame.clone()
        {
            return Ok(frame);
        }
        let frame = if self.manifest_ticks_emitted < self.manifest_ticks {
            let frame = DisplayFrame {
                bytes: self.manifest_carousel.next_frame(),
                phase: PlaybackPhase::Manifest,
                logical_index: u64::from(self.manifest_ticks_emitted),
                ecc: EccLevel::M,
            };
            self.manifest_ticks_emitted += 1;
            frame
        } else {
            if self.manifest_round_remaining == 0
                && self.ticks_since_manifest_round >= self.manifest_interval_ticks
            {
                self.manifest_round_remaining = self.manifest_carousel.round_len();
                self.ticks_since_manifest_round = 0;
            }
            if self.manifest_round_remaining > 0 {
                self.manifest_round_remaining -= 1;
                DisplayFrame {
                    bytes: self.manifest_carousel.next_frame(),
                    phase: PlaybackPhase::Manifest,
                    logical_index: self.sender.timeline_mut().current_frame(),
                    ecc: EccLevel::M,
                }
            } else {
                let logical_index = self.sender.timeline_mut().current_frame();
                let bytes = self
                    .sender
                    .next_frames(&[ChannelRequest {
                        channel_id: STABLE_CHANNEL_ID,
                        profile_id: STABLE_PROFILE_ID,
                        symbols_per_frame: 1,
                    }])?
                    .remove(0);
                self.ticks_since_manifest_round += 1;
                DisplayFrame {
                    bytes,
                    phase: PlaybackPhase::Data,
                    logical_index,
                    ecc: EccLevel::M,
                }
            }
        };
        self.last_frame = Some(frame.clone());
        Ok(frame)
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn seek_back(&mut self, frames: usize) -> u64 {
        self.last_frame = None;
        self.sender.timeline_mut().seek_back(frames)
    }

    pub fn seek_forward(&mut self, frames: usize) -> u64 {
        self.last_frame = None;
        self.sender.timeline_mut().seek_forward(frames)
    }

    /// Requests one full manifest round without moving the data timeline.
    pub fn home(&mut self) {
        self.manifest_round_remaining = self.manifest_carousel.round_len();
        self.ticks_since_manifest_round = 0;
        self.last_frame = None;
    }

    #[must_use]
    pub fn current_data_frame(&mut self) -> u64 {
        self.sender.timeline_mut().current_frame()
    }
}
