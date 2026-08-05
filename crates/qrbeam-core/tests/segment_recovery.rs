use qrbeam_core::constants::{SEGMENT_BYTES, SYMBOL_BYTES};
use qrbeam_core::error::ProtocolError;
use qrbeam_core::segment::{SegmentDecoder, SegmentEncoder, SegmentUpdate, SymbolPacket};

fn data_with_length(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

#[test]
fn all_systematic_symbols_restore_a_segment() {
    let data = data_with_length(500_123);
    let encoder = SegmentEncoder::new(3, &data).unwrap();
    let mut decoder = SegmentDecoder::new(3, data.len(), crc32c::crc32c(&data)).unwrap();

    for packet in encoder.source_packets() {
        if let SegmentUpdate::Complete(restored) = decoder.push(packet).unwrap() {
            assert_eq!(restored, data);
            return;
        }
    }
    panic!("all source symbols did not complete the segment");
}

#[test]
fn repair_symbols_restore_half_of_missing_source_symbols() {
    let data = data_with_length(500_123);
    let encoder = SegmentEncoder::new(3, &data).unwrap();
    let mut decoder = SegmentDecoder::new(3, data.len(), crc32c::crc32c(&data)).unwrap();

    for packet in encoder.source_packets().into_iter().step_by(2) {
        decoder.push(packet).unwrap();
    }
    for packet in encoder
        .repair_packets(0, encoder.source_symbol_count() / 2 + 64)
        .unwrap()
    {
        if let SegmentUpdate::Complete(restored) = decoder.push(packet).unwrap() {
            assert_eq!(restored, data);
            return;
        }
    }
    panic!("repair symbols did not complete the segment");
}

#[test]
fn source_symbols_can_arrive_in_reverse_order() {
    let data = data_with_length(16_777);
    let encoder = SegmentEncoder::new(4, &data).unwrap();
    let mut decoder = SegmentDecoder::new(4, data.len(), crc32c::crc32c(&data)).unwrap();
    let mut packets = encoder.source_packets();
    packets.reverse();

    for packet in packets {
        if let SegmentUpdate::Complete(restored) = decoder.push(packet).unwrap() {
            assert_eq!(restored, data);
            return;
        }
    }
    panic!("reversed source symbols did not complete the segment");
}

#[test]
fn repeated_esi_is_reported_as_duplicate() {
    let data = data_with_length(1_024);
    let encoder = SegmentEncoder::new(1, &data).unwrap();
    let packet = encoder.source_packets().remove(0);
    let mut decoder = SegmentDecoder::new(1, data.len(), crc32c::crc32c(&data)).unwrap();

    assert_eq!(
        decoder.push(packet.clone()).unwrap(),
        SegmentUpdate::Accepted { unique_symbols: 1 }
    );
    assert_eq!(decoder.push(packet).unwrap(), SegmentUpdate::Duplicate);
}

#[test]
fn corrupted_symbol_cannot_complete_a_crc_checked_segment() {
    let data = data_with_length(4_096);
    let encoder = SegmentEncoder::new(2, &data).unwrap();
    let mut packets = encoder.source_packets();
    packets[0].data[0] ^= 0x80;
    let mut decoder = SegmentDecoder::new(2, data.len(), crc32c::crc32c(&data)).unwrap();
    let mut final_result = Ok(SegmentUpdate::Duplicate);

    for packet in packets {
        final_result = decoder.push(packet);
    }

    assert!(matches!(
        final_result,
        Err(ProtocolError::SegmentCrcMismatch { .. })
    ));
}

#[test]
fn final_partial_symbol_is_truncated_to_actual_segment_length() {
    let data = data_with_length(777);
    let encoder = SegmentEncoder::new(5, &data).unwrap();
    let mut decoder = SegmentDecoder::new(5, data.len(), crc32c::crc32c(&data)).unwrap();

    for packet in encoder.source_packets() {
        if let SegmentUpdate::Complete(restored) = decoder.push(packet).unwrap() {
            assert_eq!(restored.len(), 777);
            assert_eq!(restored, data);
            return;
        }
    }
    panic!("partial final symbol did not complete the segment");
}

#[test]
fn segment_larger_than_protocol_limit_is_rejected() {
    let data = vec![0; SEGMENT_BYTES + 1];

    assert!(matches!(
        SegmentEncoder::new(0, &data),
        Err(ProtocolError::SegmentLengthOutOfRange { .. })
    ));
}

#[test]
fn symbol_payload_must_be_exactly_256_bytes() {
    let data = data_with_length(512);
    let mut decoder = SegmentDecoder::new(0, data.len(), crc32c::crc32c(&data)).unwrap();
    let packet = SymbolPacket {
        esi: 0,
        data: vec![0; SYMBOL_BYTES - 1],
    };

    assert_eq!(
        decoder.push(packet),
        Err(ProtocolError::InvalidSymbolPayloadLength {
            actual: SYMBOL_BYTES - 1,
            expected: SYMBOL_BYTES,
        })
    );
}
