use qrcodegen::{Mask, QrCode, QrCodeEcc, QrSegment, Version};
use thiserror::Error;

const QUIET_ZONE_MODULES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QrEcc {
    Low,
    Medium,
}

impl From<QrEcc> for QrCodeEcc {
    fn from(value: QrEcc) -> Self {
        match value {
            QrEcc::Low => Self::Low,
            QrEcc::Medium => Self::Medium,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RenderError {
    #[error("binary QR payload is too large: {actual} bytes")]
    PayloadTooLarge { actual: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QrMatrix {
    size: usize,
    modules: Vec<bool>,
    mask: u8,
}

impl QrMatrix {
    /// Encodes arbitrary binary bytes using the protocol's fixed mask 4.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::PayloadTooLarge`] when the payload cannot fit in
    /// a standard Version 40 QR code at the requested error correction level.
    pub fn encode(bytes: &[u8], ecc: QrEcc) -> Result<Self, RenderError> {
        let segments = [QrSegment::make_bytes(bytes)];
        let qr = QrCode::encode_segments_advanced(
            &segments,
            ecc.into(),
            Version::MIN,
            Version::MAX,
            Some(Mask::new(4)),
            false,
        )
        .map_err(|_| RenderError::PayloadTooLarge {
            actual: bytes.len(),
        })?;
        let qr_size = usize::try_from(qr.size()).map_err(|_| RenderError::PayloadTooLarge {
            actual: bytes.len(),
        })?;
        let size = qr_size + QUIET_ZONE_MODULES * 2;
        let mut modules = vec![false; size * size];
        for y in 0..qr_size {
            for x in 0..qr_size {
                let qr_x = i32::try_from(x).map_err(|_| RenderError::PayloadTooLarge {
                    actual: bytes.len(),
                })?;
                let qr_y = i32::try_from(y).map_err(|_| RenderError::PayloadTooLarge {
                    actual: bytes.len(),
                })?;
                let target = (y + QUIET_ZONE_MODULES) * size + x + QUIET_ZONE_MODULES;
                modules[target] = qr.get_module(qr_x, qr_y);
            }
        }
        Ok(Self {
            size,
            modules,
            mask: qr.mask().value(),
        })
    }

    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    pub const fn mask(&self) -> u8 {
        self.mask
    }

    #[must_use]
    pub fn module(&self, x: usize, y: usize) -> bool {
        self.modules
            .get(y.saturating_mul(self.size).saturating_add(x))
            .copied()
            .unwrap_or(false)
    }

    #[must_use]
    pub fn render_ansi(&self) -> String {
        let mut output = String::with_capacity(self.size * self.size * 8);
        for upper_y in (0..self.size).step_by(2) {
            let lower_y = upper_y + 1;
            let mut active_pair = None;
            for x in 0..self.size {
                let pair = (
                    self.module(x, upper_y),
                    lower_y < self.size && self.module(x, lower_y),
                );
                if active_pair != Some(pair) {
                    output.push_str(color_pair(pair));
                    active_pair = Some(pair);
                }
                output.push('▀');
            }
            output.push_str("\x1b[0m\n");
        }
        output
    }
}

const fn color_pair(pair: (bool, bool)) -> &'static str {
    match pair {
        (true, true) => "\x1b[30;40m",
        (true, false) => "\x1b[30;47m",
        (false, true) => "\x1b[37;40m",
        (false, false) => "\x1b[37;47m",
    }
}
