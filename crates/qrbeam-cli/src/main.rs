use std::error::Error;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use qrbeam_cli::player::{PlaybackPhase, Player};
use qrbeam_cli::render::{QrEcc, QrMatrix};
use qrbeam_cli::terminal::{RawMode, TerminalSurface, write_raw_line};
use qrbeam_core::manifest::EccLevel;
use qrbeam_core::session::SendSession;

const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("qrbeam: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty()
        || args
            .iter()
            .any(|argument| argument == "-h" || argument == "--help")
    {
        print_help();
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("send") {
        return Err("expected `qrbeam send <file>`".into());
    }
    let (path, fps) = parse_send_args(&args[1..])?;
    send(&path, fps)
}

fn parse_send_args(args: &[String]) -> Result<(PathBuf, u8), Box<dyn Error>> {
    let path = args
        .first()
        .filter(|value| !value.starts_with('-'))
        .map(PathBuf::from)
        .ok_or("missing file path")?;
    let mut fps = 8_u8;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--fps" => {
                let value = args.get(index + 1).ok_or("--fps requires a value")?;
                fps = value.parse()?;
                index += 2;
            }
            unknown => return Err(format!("unknown argument `{unknown}`").into()),
        }
    }
    if !(1..=8).contains(&fps) {
        return Err(format!("--fps must be between 1 and 8, got {fps}").into());
    }
    Ok((path, fps))
}

fn send(path: &Path, fps: u8) -> Result<(), Box<dyn Error>> {
    if !io::stdout().is_terminal() {
        return Err("terminal sender requires an interactive TTY".into());
    }
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "file is {} bytes; Alpha 1 limit is {MAX_FILE_BYTES} bytes",
            metadata.len()
        )
        .into());
    }
    let data = fs::read(path)?;
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("received.bin");
    let sender = SendSession::new(
        filename,
        "application/octet-stream",
        &data,
        rand::random(),
        rand::random(),
    )?;
    let mut player = Player::new(sender, fps)?;
    let frame_duration = Duration::from_secs_f64(1.0 / f64::from(fps));
    let _raw_mode = RawMode::enter()?;
    let mut terminal = TerminalSurface::enter(io::stdout())?;

    loop {
        let frame = player.next_frame()?;
        let matrix = QrMatrix::encode(
            &frame.bytes,
            match frame.ecc {
                EccLevel::L => QrEcc::Low,
                EccLevel::M => QrEcc::Medium,
            },
        )?;
        ensure_terminal_fits(&matrix)?;
        execute!(terminal.writer_mut(), MoveTo(0, 0), Clear(ClearType::All))?;
        terminal
            .writer_mut()
            .write_all(matrix.render_ansi().as_bytes())?;
        write_raw_line(
            terminal.writer_mut(),
            format_args!(
                "QRBeam Alpha 1  {}  frame {}  {} FPS  {}",
                match frame.phase {
                    PlaybackPhase::Manifest => "清单",
                    PlaybackPhase::Data => "数据",
                },
                frame.logical_index,
                fps,
                if player.is_paused() {
                    "已暂停"
                } else {
                    "发送中"
                }
            ),
        )?;
        write_raw_line(
            terminal.writer_mut(),
            format_args!("Space 暂停  J/L ±100 帧  Home 重发清单  Q/Esc 退出"),
        )?;
        terminal.writer_mut().flush()?;

        if event::poll(frame_duration)? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char(' ') => player.toggle_pause(),
                KeyCode::Char('j' | 'J') => {
                    player.seek_back(100);
                }
                KeyCode::Char('l' | 'L') => {
                    player.seek_forward(100);
                }
                KeyCode::Home => player.home(),
                KeyCode::Char('q' | 'Q') | KeyCode::Esc => break,
                _ => {}
            }
        }
    }
    Ok(())
}

fn ensure_terminal_fits(matrix: &QrMatrix) -> Result<(), Box<dyn Error>> {
    let (columns, rows) = terminal::size()?;
    let required_columns = u16::try_from(matrix.size())?;
    let required_rows = u16::try_from(matrix.size().div_ceil(2) + 2)?;
    if columns < required_columns || rows < required_rows {
        return Err(format!(
            "terminal is {columns}x{rows}; this frame needs at least {required_columns}x{required_rows}"
        )
        .into());
    }
    Ok(())
}

fn print_help() {
    println!(
        "QRBeam Alpha 1\n\nUSAGE:\n    qrbeam send <file> [--fps 1..8]\n\nSHORTCUTS:\n    Space       Pause/resume\n    J / L       Back/forward 100 frames\n    Home        Resend manifest and rewind\n    Q / Esc     Quit"
    );
}
