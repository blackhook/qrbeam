#[test]
fn package_name_matches_cargokit_artifact_name() {
    assert_eq!(
        env!("CARGO_PKG_NAME"),
        "rust_lib_qrbeam_mobile",
        "Cargokit and the Flutter plugin must agree on the library filename"
    );
}
