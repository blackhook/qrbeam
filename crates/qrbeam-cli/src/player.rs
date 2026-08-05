use qrbeam_core::error::ProtocolError;
use qrbeam_core::manifest::EccLevel;
use qrbeam_core::session::SendSession;
use qrbeam_core::timeline::ChannelRequest;
use thiserror::Error;

const MANIFEST_SECONDS: u32 = 3;
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
    manifest_frames: Vec<Vec<u8>>,
    manifest_ticks: u32,
    manifest_ticks_emitted: u32,
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
        Ok(Self {
            sender,
            manifest_frames,
            manifest_ticks: u32::from(fps) * MANIFEST_SECONDS,
            manifest_ticks_emitted: 0,
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
            let index = usize::try_from(self.manifest_ticks_emitted)
                .map_err(|_| ProtocolError::InvalidTimeline("manifest index overflow"))?
                % self.manifest_frames.len();
            let frame = DisplayFrame {
                bytes: self.manifest_frames[index].clone(),
                phase: PlaybackPhase::Manifest,
                logical_index: u64::from(self.manifest_ticks_emitted),
                ecc: EccLevel::M,
            };
            self.manifest_ticks_emitted += 1;
            frame
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
            DisplayFrame {
                bytes,
                phase: PlaybackPhase::Data,
                logical_index,
                ecc: EccLevel::M,
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

    /// Restarts the manifest stage and rewinds the existing data timeline.
    ///
    /// # Errors
    ///
    /// Returns [`PlayerError`] only if the timeline rejects frame zero.
    pub fn home(&mut self) -> Result<(), PlayerError> {
        self.manifest_ticks_emitted = 0;
        self.last_frame = None;
        self.paused = false;
        self.sender.timeline_mut().seek_frame(0)?;
        Ok(())
    }

    #[must_use]
    pub fn current_data_frame(&mut self) -> u64 {
        self.sender.timeline_mut().current_frame()
    }
}
