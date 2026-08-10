use qrbeam_core::session::SendSession;
use qrbeam_core::timeline::ChannelRequest;
use rust_lib_qrbeam_mobile::api::receiver::{MobileBlockKind, MobilePhase, MobileReceiver};

const CHANNEL: ChannelRequest = ChannelRequest {
    channel_id: 0,
    profile_id: 2,
    symbols_per_frame: 8,
};

fn manifest_frames(sender: &SendSession) -> Vec<Vec<u8>> {
    sender
        .manifest()
        .fragments()
        .unwrap()
        .into_iter()
        .enumerate()
        .map(|(index, fragment)| {
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
fn bridge_reports_manifest_blocks_and_completed_bytes() {
    let data: Vec<u8> = (0..8_193)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    let mut sender = SendSession::new("bridge.bin", "", &data, [0x61; 16], 61).unwrap();
    let mut receiver = MobileReceiver::new();

    assert_eq!(receiver.snapshot().phase, MobilePhase::WaitingManifest);
    for frame in manifest_frames(&sender) {
        receiver.ingest(frame).unwrap();
    }
    let ready = receiver.snapshot();
    assert_eq!(ready.phase, MobilePhase::Receiving);
    assert_eq!(ready.filename.as_deref(), Some("bridge.bin"));
    assert_eq!(ready.total_bytes, Some(8_193));
    assert_eq!(ready.blocks[0].kind, MobileBlockKind::Missing);

    for _ in 0..100 {
        let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);
        let snapshot = receiver.ingest(frame).unwrap();
        if snapshot.phase == MobilePhase::Complete {
            break;
        }
    }

    assert_eq!(receiver.snapshot().phase, MobilePhase::Complete);
    assert_eq!(
        receiver.snapshot().blocks[0].kind,
        MobileBlockKind::Complete
    );
    assert_eq!(receiver.completed_file(), Some(data));
}
