use qrbeam_core::constants::{
    FRAME_HEADER_BYTES, MAX_FRAME_PAYLOAD_BYTES, PROTOCOL_VERSION, SYMBOL_BYTES,
};
use qrbeam_core::error::ProtocolError;
use qrbeam_core::frame::{Frame, FrameHeader, FrameType};

fn data_frame() -> Frame {
    Frame {
        header: FrameHeader {
            frame_type: FrameType::Data,
            flags: 0,
            channel_id: 2,
            profile_id: 3,
            session_id: [0x11; 16],
            file_id: 0x0102_0304,
            global_frame_index: 0x0102_0304_0506_0708,
            segment_index: 9,
            first_symbol_id: 10,
            symbol_count: 1,
        },
        payload: vec![0xA5; SYMBOL_BYTES],
    }
}

fn rewrite_crc(bytes: &mut [u8]) {
    bytes[52..56].fill(0);
    let crc = crc32c::crc32c(bytes);
    bytes[52..56].copy_from_slice(&crc.to_le_bytes());
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn frame_round_trips_with_fixed_little_endian_offsets() {
    let frame = data_frame();

    let bytes = frame.encode().unwrap();

    assert_eq!(&bytes[0..4], b"QRBM");
    assert_eq!(bytes[4], PROTOCOL_VERSION);
    assert_eq!(bytes[5], FrameType::Data as u8);
    assert_eq!(bytes.len(), FRAME_HEADER_BYTES + SYMBOL_BYTES);
    assert_eq!(u16::from_le_bytes(bytes[10..12].try_into().unwrap()), 56);
    assert_eq!(
        u32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        0x0102_0304
    );
    assert_eq!(
        u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
        0x0102_0304_0506_0708
    );
    assert_eq!(Frame::decode(&bytes).unwrap(), frame);
}

#[test]
fn payload_bit_flip_is_rejected_by_crc32c() {
    let mut bytes = data_frame().encode().unwrap();
    bytes[FRAME_HEADER_BYTES + 17] ^= 0x01;

    assert!(matches!(
        Frame::decode(&bytes),
        Err(ProtocolError::CrcMismatch { .. })
    ));
}

#[test]
fn unknown_frame_type_is_rejected_after_crc_validation() {
    let mut bytes = data_frame().encode().unwrap();
    bytes[5] = 0xFF;
    rewrite_crc(&mut bytes);

    assert_eq!(
        Frame::decode(&bytes),
        Err(ProtocolError::UnknownFrameType(0xFF))
    );
}

#[test]
fn payload_larger_than_qr_capacity_is_rejected() {
    let frame = Frame {
        header: FrameHeader {
            frame_type: FrameType::Test,
            flags: 0,
            channel_id: 0,
            profile_id: 0,
            session_id: [0; 16],
            file_id: 1,
            global_frame_index: 0,
            segment_index: 0,
            first_symbol_id: 0,
            symbol_count: 0,
        },
        payload: vec![0; MAX_FRAME_PAYLOAD_BYTES + 1],
    };

    assert_eq!(
        frame.encode(),
        Err(ProtocolError::PayloadTooLarge {
            actual: MAX_FRAME_PAYLOAD_BYTES + 1,
            maximum: MAX_FRAME_PAYLOAD_BYTES,
        })
    );
}

#[test]
fn symbol_frame_payload_must_match_symbol_count() {
    let mut frame = data_frame();
    frame.header.symbol_count = 2;

    assert_eq!(
        frame.encode(),
        Err(ProtocolError::SymbolPayloadLengthMismatch {
            expected: SYMBOL_BYTES * 2,
            actual: SYMBOL_BYTES,
        })
    );
}

#[test]
fn truncated_frame_is_rejected_without_panicking() {
    assert_eq!(
        Frame::decode(&[0; 12]),
        Err(ProtocolError::FrameTooShort {
            actual: 12,
            minimum: FRAME_HEADER_BYTES,
        })
    );
}

#[test]
fn frame_encoding_matches_independent_golden_bytes() {
    let mut expected = decode_hex(concat!(
        "5152424d0103000203003800",
        "11111111111111111111111111111111",
        "04030201",
        "0807060504030201",
        "09000000",
        "0a000000",
        "0100",
        "0001",
        "34387042",
    ));
    expected.extend([0xA5; SYMBOL_BYTES]);

    assert_eq!(data_frame().encode().unwrap(), expected);
}
