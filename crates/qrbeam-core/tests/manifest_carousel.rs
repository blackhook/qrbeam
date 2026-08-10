use qrbeam_core::manifest::ManifestFragment;
use qrbeam_core::manifest_carousel::ManifestCarousel;
use qrbeam_core::session::SendSession;

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
fn carousel_repeats_the_complete_manifest_without_advancing_data_cursor() {
    let sender = SendSession::new("sample.bin", "", &[7; 1_024], [1; 16], 1).unwrap();
    let mut carousel = ManifestCarousel::new(manifest_frames(&sender)).unwrap();

    let first_round = carousel.take_round();
    let second_round = carousel.take_round();

    assert_eq!(first_round, second_round);
}
