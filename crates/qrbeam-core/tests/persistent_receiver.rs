use qrbeam_core::persistent_receiver::{PersistentReceiver, PersistentUpdate};
use qrbeam_core::session::{BlockState, SendSession};
use qrbeam_core::timeline::ChannelRequest;

const CHANNEL: ChannelRequest = ChannelRequest {
    channel_id: 0,
    profile_id: 2,
    symbols_per_frame: 8,
};

fn sender() -> SendSession {
    SendSession::new("persist.bin", "", &[7; 1_024], [0x71; 16], 71).unwrap()
}

#[test]
fn recovered_segment_is_not_complete_until_storage_acknowledges_it() {
    let mut sender = sender();
    let mut receiver = PersistentReceiver::from_manifest(sender.manifest().clone()).unwrap();
    let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);

    let update = receiver.ingest(&frame).unwrap();

    assert!(matches!(
        update,
        PersistentUpdate::SegmentReady { index: 0, .. }
    ));
    assert!(matches!(
        receiver.snapshot().blocks.as_slice(),
        [BlockState::Partial { .. }]
    ));
    receiver.acknowledge_segment(0).unwrap();
    assert_eq!(receiver.snapshot().blocks, vec![BlockState::Complete]);
}

#[test]
fn replaying_saved_frames_restores_a_partial_segment() {
    let mut sender = sender();
    let frame = sender.next_frames(&[CHANNEL]).unwrap().remove(0);
    let mut restored = PersistentReceiver::from_manifest(sender.manifest().clone()).unwrap();

    restored.restore_partial_frames(0, &[frame]).unwrap();

    assert!(matches!(
        restored.snapshot().blocks.as_slice(),
        [BlockState::Partial { .. }]
    ));
}
