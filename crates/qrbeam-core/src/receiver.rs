use crate::error::ProtocolError;
use crate::frame::{Frame, FrameType};
use crate::manifest::ManifestAssembler;
use crate::session::{BlockState, ReceiveSession, ReceiveUpdate};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiverPhase {
    WaitingManifest,
    Receiving,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiverSnapshot {
    pub phase: ReceiverPhase,
    pub filename: Option<String>,
    pub total_bytes: Option<u64>,
    pub blocks: Vec<BlockState>,
    pub last_frame_index: Option<u64>,
}

impl Default for ReceiverSnapshot {
    fn default() -> Self {
        Self {
            phase: ReceiverPhase::WaitingManifest,
            filename: None,
            total_bytes: None,
            blocks: Vec::new(),
            last_frame_index: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerUpdate {
    ManifestProgress,
    ManifestReady,
    IgnoredDuplicate,
    Accepted,
    BlockComplete(u32),
    Complete,
}

#[derive(Debug, Default)]
pub struct ReceiverController {
    assembler: ManifestAssembler,
    session: Option<ReceiveSession>,
    snapshot: ReceiverSnapshot,
}

impl ReceiverController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates and ingests one QRBeam wire frame.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the frame, manifest, session identity,
    /// RaptorQ symbol range, segment checksum, or final file hash is invalid.
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<ControllerUpdate, ProtocolError> {
        let frame = Frame::decode(bytes)?;
        match frame.header.frame_type {
            FrameType::Manifest => self.ingest_manifest(&frame),
            FrameType::Data | FrameType::Repair => self.ingest_data(bytes, &frame),
            FrameType::Test | FrameType::Control => Err(ProtocolError::UnexpectedFrameType),
        }
    }

    #[must_use]
    pub const fn snapshot(&self) -> &ReceiverSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn completed_file(&self) -> Option<&[u8]> {
        self.session
            .as_ref()
            .and_then(ReceiveSession::completed_file)
    }

    fn ingest_manifest(&mut self, frame: &Frame) -> Result<ControllerUpdate, ProtocolError> {
        let Some(manifest) = self.assembler.push(frame)? else {
            return Ok(ControllerUpdate::ManifestProgress);
        };
        let filename = manifest.filename.clone();
        let total_bytes = manifest.original_length;
        let session = ReceiveSession::new(manifest)?;
        let blocks = session.block_states();
        self.session = Some(session);
        self.snapshot = ReceiverSnapshot {
            phase: ReceiverPhase::Receiving,
            filename: Some(filename),
            total_bytes: Some(total_bytes),
            blocks,
            last_frame_index: Some(frame.header.global_frame_index),
        };
        Ok(ControllerUpdate::ManifestReady)
    }

    fn ingest_data(
        &mut self,
        bytes: &[u8],
        frame: &Frame,
    ) -> Result<ControllerUpdate, ProtocolError> {
        let session = self
            .session
            .as_mut()
            .ok_or(ProtocolError::ManifestRequired)?;
        let update = session.ingest(bytes)?;
        self.snapshot.blocks = session.block_states();
        self.snapshot.last_frame_index = Some(frame.header.global_frame_index);
        let controller_update = match update {
            ReceiveUpdate::IgnoredDuplicate => ControllerUpdate::IgnoredDuplicate,
            ReceiveUpdate::Accepted => ControllerUpdate::Accepted,
            ReceiveUpdate::BlockComplete(index) => ControllerUpdate::BlockComplete(index),
            ReceiveUpdate::Complete(_) => {
                self.snapshot.phase = ReceiverPhase::Complete;
                ControllerUpdate::Complete
            }
        };
        Ok(controller_update)
    }
}
