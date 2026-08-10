use qrbeam_core::receiver::{ReceiverController, ReceiverPhase, ReceiverSnapshot};
use qrbeam_core::session::BlockState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MobilePhase {
    WaitingManifest,
    Receiving,
    Complete,
}

impl From<ReceiverPhase> for MobilePhase {
    fn from(value: ReceiverPhase) -> Self {
        match value {
            ReceiverPhase::WaitingManifest => Self::WaitingManifest,
            ReceiverPhase::Receiving => Self::Receiving,
            ReceiverPhase::Complete => Self::Complete,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MobileBlockKind {
    Missing,
    Partial,
    Complete,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MobileBlockState {
    pub kind: MobileBlockKind,
    pub unique: u32,
    pub required: u32,
}

impl From<&BlockState> for MobileBlockState {
    fn from(value: &BlockState) -> Self {
        match value {
            BlockState::Missing => Self {
                kind: MobileBlockKind::Missing,
                unique: 0,
                required: 0,
            },
            BlockState::Partial { unique, required } => Self {
                kind: MobileBlockKind::Partial,
                unique: *unique,
                required: *required,
            },
            BlockState::Complete => Self {
                kind: MobileBlockKind::Complete,
                unique: 0,
                required: 0,
            },
            BlockState::Failed { unique, required } => Self {
                kind: MobileBlockKind::Failed,
                unique: *unique,
                required: *required,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MobileSnapshot {
    pub phase: MobilePhase,
    pub filename: Option<String>,
    pub total_bytes: Option<u64>,
    pub blocks: Vec<MobileBlockState>,
    pub last_frame_index: Option<u64>,
}

impl From<&ReceiverSnapshot> for MobileSnapshot {
    fn from(value: &ReceiverSnapshot) -> Self {
        Self {
            phase: value.phase.into(),
            filename: value.filename.clone(),
            total_bytes: value.total_bytes,
            blocks: value.blocks.iter().map(MobileBlockState::from).collect(),
            last_frame_index: value.last_frame_index,
        }
    }
}

#[derive(Debug, Default)]
#[flutter_rust_bridge::frb(opaque)]
pub struct MobileReceiver {
    inner: ReceiverController,
}

impl MobileReceiver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn snapshot(&self) -> MobileSnapshot {
        MobileSnapshot::from(self.inner.snapshot())
    }

    /// Ingests one decoded QR payload and returns the latest UI snapshot.
    ///
    /// # Errors
    ///
    /// Returns a display-safe protocol error when the frame is invalid.
    #[allow(clippy::needless_pass_by_value)]
    pub fn ingest(&mut self, frame: Vec<u8>) -> Result<MobileSnapshot, String> {
        self.inner
            .ingest(&frame)
            .map_err(|error| error.to_string())?;
        Ok(self.snapshot())
    }

    #[must_use]
    pub fn completed_file(&self) -> Option<Vec<u8>> {
        self.inner.completed_file().map(<[u8]>::to_vec)
    }
}
