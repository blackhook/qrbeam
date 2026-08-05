use qrbeam_core::constants::FRAME_HEADER_BYTES;
use qrbeam_core::error::ProtocolError;
use qrbeam_core::session::{BlockState, ReceiveSession, ReceiveUpdate, SendSession};
use qrbeam_core::timeline::ChannelRequest;

const CHANNEL: ChannelRequest = ChannelRequest {
    channel_id: 0,
    profile_id: 2,
    symbols_per_frame: 8,
};

fn data_with_length(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

#[derive(Debug)]
struct StreamResult {
    restored: Vec<u8>,
    generated_frames: usize,
    crc_errors: usize,
}

fn run_damaged_stream(data: &[u8], seed: u64, drop_percent: u64) -> StreamResult {
    let mut sender =
        SendSession::new("sample.bin", "application/octet-stream", data, [7; 16], 42).unwrap();
    let mut receiver = ReceiveSession::new(sender.manifest().clone()).unwrap();
    let mut rng = seed;
    let mut generated_frames = 0_usize;
    let mut crc_errors = 0_usize;
    let mut batch = Vec::new();

    for _ in 0..20_000 {
        let frames = sender.next_frames(&[CHANNEL]).unwrap();
        for mut bytes in frames {
            generated_frames += 1;
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            if rng % 100 < drop_percent {
                continue;
            }
            if generated_frames.is_multiple_of(17) {
                bytes[FRAME_HEADER_BYTES + 3] ^= 0x01;
            }
            batch.push(bytes.clone());
            if generated_frames.is_multiple_of(11) {
                batch.push(bytes);
            }
        }
        if batch.len() < 16 {
            continue;
        }
        batch.reverse();
        for bytes in batch.drain(..) {
            match receiver.ingest(&bytes) {
                Ok(ReceiveUpdate::Complete(restored)) => {
                    return StreamResult {
                        restored,
                        generated_frames,
                        crc_errors,
                    };
                }
                Ok(_) => {}
                Err(ProtocolError::CrcMismatch { .. }) => crc_errors += 1,
                Err(error) => panic!("unexpected receive error: {error}"),
            }
        }
    }
    panic!("receiver did not complete under the deterministic loss profile");
}

#[test]
fn small_file_survives_loss_duplicates_reordering_and_corruption() {
    let data = data_with_length(64 * 1024 + 123);

    let result = run_damaged_stream(&data, 0x1234_5678_9abc_def0, 35);

    assert_eq!(result.restored, data);
    assert!(result.generated_frames > 0);
    assert!(result.crc_errors > 0);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "large RaptorQ recovery runs in release")]
fn large_file_survives_deterministic_damaged_channel() {
    let data = data_with_length(1_200_123);

    let result = run_damaged_stream(&data, 0x1234_5678_9abc_def0, 50);

    assert_eq!(result.restored, data);
    assert!(result.generated_frames > 1_000);
    assert!(result.crc_errors > 0);
}

#[test]
fn frame_from_another_session_is_rejected() {
    let data = data_with_length(1_024);
    let mut sender = SendSession::new("a.bin", "", &data, [1; 16], 5).unwrap();
    let mut wrong_manifest = sender.manifest().clone();
    wrong_manifest.session_id = [2; 16];
    let mut receiver = ReceiveSession::new(wrong_manifest).unwrap();
    let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);

    assert_eq!(receiver.ingest(&frame), Err(ProtocolError::SessionMismatch));
}

#[test]
fn wrong_final_blake3_never_reports_complete() {
    let data = data_with_length(4_096);
    let mut sender = SendSession::new("a.bin", "", &data, [3; 16], 6).unwrap();
    let mut wrong_manifest = sender.manifest().clone();
    wrong_manifest.file_hash[0] ^= 0x01;
    let mut receiver = ReceiveSession::new(wrong_manifest).unwrap();
    let mut final_result = Ok(ReceiveUpdate::Accepted);

    for _ in 0..8 {
        for frame in sender.next_frames(&[CHANNEL]).unwrap() {
            final_result = receiver.ingest(&frame);
        }
        if final_result.is_err() {
            break;
        }
    }

    assert!(matches!(
        final_result,
        Err(ProtocolError::FileHashMismatch { .. })
    ));
}

#[test]
fn block_map_moves_from_missing_to_partial_and_ignores_replayed_frame() {
    let data = data_with_length(1_024);
    let mut sender = SendSession::new("a.bin", "", &data, [4; 16], 7).unwrap();
    let mut receiver = ReceiveSession::new(sender.manifest().clone()).unwrap();
    let slow_channel = ChannelRequest {
        channel_id: 0,
        profile_id: 0,
        symbols_per_frame: 1,
    };
    assert_eq!(receiver.block_states(), vec![BlockState::Missing]);
    let frame = sender.next_frames(&[slow_channel]).unwrap().remove(0);

    assert_eq!(receiver.ingest(&frame), Ok(ReceiveUpdate::Accepted));
    assert_eq!(
        receiver.block_states(),
        vec![BlockState::Partial {
            unique: 1,
            required: 4,
        }]
    );
    assert_eq!(receiver.ingest(&frame), Ok(ReceiveUpdate::IgnoredDuplicate));
}

#[test]
fn one_byte_and_one_kilobyte_round_trip_exactly() {
    for length in [1, 1_000] {
        assert_eq!(round_trip_without_loss(length), data_with_length(length));
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "one MiB RaptorQ encoding runs in release")]
fn one_megabyte_round_trips_exactly() {
    assert_eq!(
        round_trip_without_loss(1_000_000),
        data_with_length(1_000_000)
    );
}

fn round_trip_without_loss(length: usize) -> Vec<u8> {
    let data = data_with_length(length);
    let mut sender = SendSession::new("sizes.bin", "", &data, [9; 16], 99).unwrap();
    let mut receiver = ReceiveSession::new(sender.manifest().clone()).unwrap();

    for _ in 0..5_000 {
        for frame in sender.next_frames(&[CHANNEL]).unwrap() {
            if let ReceiveUpdate::Complete(file) = receiver.ingest(&frame).unwrap() {
                return file;
            }
        }
    }
    panic!("receiver did not complete {length} bytes without loss");
}
