use qrbeam_core::manifest::ManifestFragment;
use qrbeam_core::receiver::{ControllerUpdate, ReceiverController, ReceiverPhase};
use qrbeam_core::session::{BlockState, SendSession};
use qrbeam_core::timeline::ChannelRequest;

const CHANNEL: ChannelRequest = ChannelRequest {
    channel_id: 0,
    profile_id: 2,
    symbols_per_frame: 8,
};

fn sample_data(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

fn manifest_frames(sender: &SendSession) -> Vec<Vec<u8>> {
    sender
        .manifest()
        .fragments()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(index, fragment): (usize, ManifestFragment)| {
            fragment
                .into_frame(
                    sender.manifest().session_id,
                    sender.manifest().file_id,
                    u64::try_from(index).unwrap(),
                    0,
                    0,
                )
                .unwrap()
                .encode()
                .unwrap()
        })
        .collect()
}

#[test]
fn controller_moves_from_manifest_to_complete_across_a_damaged_stream() {
    let data = sample_data(64 * 1024 + 73);
    let mut sender = SendSession::new(
        "离线样本.bin",
        "application/octet-stream",
        &data,
        [0x21; 16],
        77,
    )
    .unwrap();
    let mut receiver = ReceiverController::new();

    assert_eq!(receiver.snapshot().phase, ReceiverPhase::WaitingManifest);
    assert_eq!(receiver.snapshot().filename, None);

    let mut manifest = manifest_frames(&sender);
    manifest.reverse();
    manifest.push(manifest[0].clone());
    let mut saw_manifest = false;
    for frame in manifest {
        if receiver.ingest(&frame).unwrap() == ControllerUpdate::ManifestReady {
            saw_manifest = true;
        }
    }

    assert!(saw_manifest);
    assert_eq!(receiver.snapshot().phase, ReceiverPhase::Receiving);
    assert_eq!(
        receiver.snapshot().filename.as_deref(),
        Some("离线样本.bin")
    );
    assert_eq!(receiver.snapshot().total_bytes, Some(data.len() as u64));
    assert_eq!(receiver.snapshot().blocks, vec![BlockState::Missing]);

    let first = sender.next_frames(&[CHANNEL]).unwrap().remove(0);
    assert_eq!(receiver.ingest(&first), Ok(ControllerUpdate::Accepted));
    assert!(matches!(
        receiver.snapshot().blocks.as_slice(),
        [BlockState::Partial { .. }]
    ));

    let mut completed = false;
    for generated in 1..2_000 {
        let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);
        if generated % 3 == 0 {
            continue;
        }
        if generated % 11 == 0 {
            let _ = receiver.ingest(&frame).unwrap();
        }
        if receiver.ingest(&frame).unwrap() == ControllerUpdate::Complete {
            completed = true;
            break;
        }
    }

    assert!(completed, "repair symbols should finish the damaged stream");
    assert_eq!(receiver.snapshot().phase, ReceiverPhase::Complete);
    assert_eq!(receiver.snapshot().blocks, vec![BlockState::Complete]);
    assert_eq!(receiver.completed_file(), Some(data.as_slice()));
    assert!(receiver.snapshot().last_frame_index.is_some());
}

#[test]
fn data_before_a_manifest_is_rejected_without_changing_snapshot() {
    let data = sample_data(1_024);
    let mut sender = SendSession::new("early.bin", "", &data, [0x33; 16], 88).unwrap();
    let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);
    let mut receiver = ReceiverController::new();
    let before = receiver.snapshot().clone();

    let error = receiver.ingest(&frame).unwrap_err();

    assert_eq!(
        error.to_string(),
        "a validated manifest is required before data frames"
    );
    assert_eq!(receiver.snapshot(), &before);
}
