use crate::error::ProtocolError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestCarousel {
    frames: Vec<Vec<u8>>,
    next: usize,
}

impl ManifestCarousel {
    /// Creates a cyclic manifest-frame sequence.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when no encoded manifest frame is supplied.
    pub fn new(frames: Vec<Vec<u8>>) -> Result<Self, ProtocolError> {
        if frames.is_empty() {
            return Err(ProtocolError::InvalidManifest(
                "manifest carousel requires at least one frame",
            ));
        }
        Ok(Self { frames, next: 0 })
    }

    #[must_use]
    pub fn next_frame(&mut self) -> Vec<u8> {
        let frame = self.frames[self.next].clone();
        self.next = (self.next + 1) % self.frames.len();
        frame
    }

    #[must_use]
    pub fn take_round(&mut self) -> Vec<Vec<u8>> {
        (0..self.round_len()).map(|_| self.next_frame()).collect()
    }

    #[must_use]
    pub const fn round_len(&self) -> usize {
        self.frames.len()
    }
}
