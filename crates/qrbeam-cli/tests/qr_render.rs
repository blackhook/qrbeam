use qrbeam_cli::render::{QrEcc, QrMatrix, RenderError};

fn frame_bytes() -> Vec<u8> {
    (0..312)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

#[test]
fn matrix_is_deterministic_and_uses_fixed_mask_four() {
    let first = QrMatrix::encode(&frame_bytes(), QrEcc::Medium).unwrap();
    let second = QrMatrix::encode(&frame_bytes(), QrEcc::Medium).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.mask(), 4);
}

#[test]
fn matrix_includes_a_four_module_quiet_zone() {
    let matrix = QrMatrix::encode(&frame_bytes(), QrEcc::Medium).unwrap();

    for coordinate in 0..matrix.size() {
        for edge in 0..4 {
            assert!(!matrix.module(coordinate, edge));
            assert!(!matrix.module(edge, coordinate));
            assert!(!matrix.module(coordinate, matrix.size() - 1 - edge));
            assert!(!matrix.module(matrix.size() - 1 - edge, coordinate));
        }
    }
}

#[test]
fn oversized_binary_payload_is_rejected() {
    let error = QrMatrix::encode(&vec![0xa5; 3_000], QrEcc::Low).unwrap_err();

    assert_eq!(error, RenderError::PayloadTooLarge { actual: 3_000 });
}

#[test]
fn ansi_half_blocks_pack_two_module_rows_into_one_terminal_row() {
    let matrix = QrMatrix::encode(&frame_bytes(), QrEcc::Medium).unwrap();
    let rendered = matrix.render_ansi();

    assert_eq!(rendered.lines().count(), matrix.size().div_ceil(2));
    assert!(rendered.contains('▀'));
}
