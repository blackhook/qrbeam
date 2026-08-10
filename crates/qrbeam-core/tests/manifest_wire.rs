use qrbeam_core::constants::{MAX_FILE_BYTES, SEGMENT_BYTES_U32, SYMBOL_BYTES_U16};
use qrbeam_core::error::ProtocolError;
use qrbeam_core::frame::Frame;
use qrbeam_core::manifest::{Compression, Manifest, ManifestAssembler, Profile};

fn sample_manifest() -> Manifest {
    Manifest {
        session_id: [0x22; 16],
        file_id: 7,
        filename: "example.bin".into(),
        mime_type: "application/octet-stream".into(),
        original_length: 1_000_000,
        container_length: 1_000_000,
        compression: Compression::None,
        file_hash: [0x33; 32],
        segment_size: SEGMENT_BYTES_U32,
        symbol_size: SYMBOL_BYTES_U16,
        last_segment_length: 475_712,
        encoding_seed: [0x44; 16],
        profiles: Profile::defaults().to_vec(),
        segment_crc32c: vec![0x1234_5678, 0x90ab_cdef],
    }
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn manifest_round_trips_with_all_profiles_and_segment_checksums() {
    let manifest = sample_manifest();

    let encoded = manifest.encode().unwrap();
    let decoded = Manifest::decode(&encoded).unwrap();

    assert_eq!(decoded, manifest);
    assert_eq!(decoded.profiles, Profile::defaults());
}

#[test]
fn high_profile_accepts_eleven_symbols_per_frame() {
    let manifest = sample_manifest();

    assert_eq!(manifest.profiles[3].symbols_per_frame, 11);
    manifest.encode().unwrap();
}

#[test]
fn manifest_rejects_a_filename_longer_than_255_utf8_bytes() {
    let mut manifest = sample_manifest();
    manifest.filename = "a".repeat(256);

    assert!(manifest.encode().is_err());
}

#[test]
fn large_manifest_fragments_reassemble_out_of_order() {
    let mut manifest = sample_manifest();
    manifest.original_length = MAX_FILE_BYTES;
    manifest.container_length = MAX_FILE_BYTES;
    manifest.last_segment_length = 385_280;
    manifest.segment_crc32c = (0..191).map(|index| index ^ 0xA5A5_0000).collect();

    let fragments = manifest.fragments().unwrap();
    assert!(fragments.len() > 1);
    let mut frames: Vec<_> = fragments
        .into_iter()
        .enumerate()
        .map(|(index, fragment)| {
            fragment
                .into_frame(manifest.session_id, manifest.file_id, index as u64, 0, 0)
                .unwrap()
        })
        .collect();
    frames.reverse();

    let mut assembler = ManifestAssembler::new();
    let mut restored = None;
    for frame in &frames {
        if let Some(value) = assembler.push(frame).unwrap() {
            restored = Some(value);
        }
    }

    assert_eq!(restored, Some(manifest));
}

#[test]
fn duplicate_fragment_does_not_advance_received_count() {
    let manifest = sample_manifest();
    let fragment = manifest.fragments().unwrap().remove(0);
    let frame = fragment
        .into_frame(manifest.session_id, manifest.file_id, 0, 0, 0)
        .unwrap();
    let mut assembler = ManifestAssembler::new();

    let first = assembler.push(&frame).unwrap();
    let count_after_first = assembler.received_count();
    let second = assembler.push(&frame).unwrap();

    assert_eq!(first, Some(manifest));
    assert_eq!(second, None);
    assert_eq!(assembler.received_count(), count_after_first);
}

#[test]
fn missing_fragment_never_returns_a_manifest() {
    let mut manifest = sample_manifest();
    manifest.segment_crc32c = vec![0x0102_0304; 191];
    manifest.original_length = MAX_FILE_BYTES;
    manifest.container_length = MAX_FILE_BYTES;
    manifest.last_segment_length = 385_280;
    let mut fragments = manifest.fragments().unwrap();
    fragments.pop();
    let mut assembler = ManifestAssembler::new();

    for (index, fragment) in fragments.into_iter().enumerate() {
        let frame = fragment
            .into_frame(manifest.session_id, manifest.file_id, index as u64, 0, 0)
            .unwrap();
        assert_eq!(assembler.push(&frame).unwrap(), None);
    }
}

#[test]
fn file_above_hard_limit_is_rejected() {
    let mut manifest = sample_manifest();
    manifest.original_length = MAX_FILE_BYTES + 1;

    assert_eq!(
        manifest.encode(),
        Err(ProtocolError::FileTooLarge {
            actual: MAX_FILE_BYTES + 1,
            maximum: MAX_FILE_BYTES,
        })
    );
}

#[test]
fn changed_fragment_with_valid_frame_crc_fails_manifest_hash() {
    let mut manifest = sample_manifest();
    manifest.segment_crc32c = vec![0xABCD_1234; 191];
    manifest.original_length = MAX_FILE_BYTES;
    manifest.container_length = MAX_FILE_BYTES;
    manifest.last_segment_length = 385_280;
    let fragments = manifest.fragments().unwrap();
    let mut frames: Vec<_> = fragments
        .into_iter()
        .enumerate()
        .map(|(index, fragment)| {
            fragment
                .into_frame(manifest.session_id, manifest.file_id, index as u64, 0, 0)
                .unwrap()
        })
        .collect();
    frames[0].payload[40] ^= 0x01;
    frames[0] = Frame::decode(&frames[0].encode().unwrap()).unwrap();
    let mut assembler = ManifestAssembler::new();

    for frame in &frames[..frames.len() - 1] {
        assert_eq!(assembler.push(frame).unwrap(), None);
    }
    assert_eq!(
        assembler.push(frames.last().unwrap()),
        Err(ProtocolError::ManifestHashMismatch)
    );
}

#[test]
fn changed_manifest_bytes_fail_internal_crc() {
    let mut encoded = sample_manifest().encode().unwrap();
    encoded[30] ^= 0x80;

    assert!(matches!(
        Manifest::decode(&encoded),
        Err(ProtocolError::ManifestCrcMismatch { .. })
    ));
}

#[test]
fn manifest_encoding_matches_independent_golden_bytes() {
    let expected = decode_hex(concat!(
        "51524d4601000000",
        "22222222222222222222222222222222",
        "07000000",
        "40420f0000000000",
        "40420f0000000000",
        "3333333333333333333333333333333333333333333333333333333333333333",
        "00000800",
        "0001",
        "04",
        "00",
        "40420700",
        "44444444444444444444444444444444",
        "0b00",
        "1800",
        "02000000",
        "6578616d706c652e62696e",
        "6170706c69636174696f6e2f6f637465742d73747265616d",
        "0001010806280000",
        "0105001806280000",
        "0208001e04280000",
        "030b003c04280000",
        "78563412",
        "efcdab90",
        "6ed126ae",
    ));

    assert_eq!(sample_manifest().encode().unwrap(), expected);
}
