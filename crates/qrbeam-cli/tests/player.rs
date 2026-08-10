use qrbeam_cli::player::{PlaybackPhase, Player};
use qrbeam_core::frame::Frame;
use qrbeam_core::session::SendSession;

fn sender() -> SendSession {
    let data: Vec<u8> = (0..4_096)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    SendSession::new("player.bin", "application/octet-stream", &data, [9; 16], 5).unwrap()
}

#[test]
fn manifest_repeats_for_three_seconds_before_data_starts() {
    let mut player = Player::new(sender(), 4).unwrap();

    for _ in 0..12 {
        assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Manifest);
    }
    assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Data);
}

#[test]
fn player_repeats_a_manifest_round_every_two_seconds_after_warmup() {
    let mut player = Player::new(sender(), 4).unwrap();

    for _ in 0..12 {
        assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Manifest);
    }
    for _ in 0..8 {
        assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Data);
    }

    assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Manifest);
}

#[test]
fn pause_repeats_the_visible_frame_without_advancing() {
    let mut player = Player::new(sender(), 1).unwrap();
    for _ in 0..3 {
        player.next_frame().unwrap();
    }
    let first = player.next_frame().unwrap();
    player.toggle_pause();

    let paused = player.next_frame().unwrap();
    assert_eq!(paused, first);
    assert!(player.is_paused());

    player.toggle_pause();
    let resumed = player.next_frame().unwrap();
    assert_ne!(resumed.bytes, first.bytes);
}

#[test]
fn seek_back_replays_historical_data_frames() {
    let mut player = Player::new(sender(), 1).unwrap();
    for _ in 0..3 {
        player.next_frame().unwrap();
    }
    for _ in 0..400 {
        player.next_frame().unwrap();
    }
    let live_edge = player.current_data_frame();

    assert_eq!(player.seek_back(100), live_edge - 100);
    let replayed = player.next_frame().unwrap();
    let decoded = Frame::decode(&replayed.bytes).unwrap();
    assert_eq!(decoded.header.global_frame_index, live_edge - 100);
}

#[test]
fn home_requests_a_manifest_round_without_rewinding_data_timeline() {
    let mut player = Player::new(sender(), 1).unwrap();
    for _ in 0..20 {
        player.next_frame().unwrap();
    }
    let before_home = player.current_data_frame();

    player.home();

    assert_eq!(player.current_data_frame(), before_home);
    assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Manifest);
    assert_eq!(player.next_frame().unwrap().phase, PlaybackPhase::Data);
    assert_eq!(player.current_data_frame(), before_home + 1);
}
