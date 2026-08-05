use std::io::{self, Write};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

pub struct RawMode;

impl RawMode {
    /// Enables terminal raw mode until the returned guard is dropped.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when raw mode cannot be enabled.
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

pub struct TerminalSurface<W: Write> {
    writer: W,
}

impl<W: Write> TerminalSurface<W> {
    /// Enters the terminal alternate screen and hides the cursor.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the control sequences cannot be written.
    pub fn enter(mut writer: W) -> io::Result<Self> {
        execute!(writer, EnterAlternateScreen, Hide)?;
        Ok(Self { writer })
    }

    pub const fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W: Write> Drop for TerminalSurface<W> {
    fn drop(&mut self) {
        let _ = execute!(self.writer, Show, LeaveAlternateScreen);
    }
}
