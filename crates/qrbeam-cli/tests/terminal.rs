use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use qrbeam_cli::terminal::TerminalSurface;

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn dropping_terminal_surface_restores_cursor_and_primary_screen() {
    let writer = SharedWriter::default();
    let captured = writer.0.clone();

    {
        let _surface = TerminalSurface::enter(writer).unwrap();
    }

    let output = String::from_utf8(captured.lock().unwrap().clone()).unwrap();
    assert!(
        output.contains("\x1b[?1049h"),
        "must enter alternate screen"
    );
    assert!(output.contains("\x1b[?25l"), "must hide cursor");
    assert!(output.contains("\x1b[?25h"), "must restore cursor");
    assert!(
        output.contains("\x1b[?1049l"),
        "must leave alternate screen"
    );
}
