use std::path::PathBuf;

use mortimmy_integration_test::load_hardware_test_config_from_path;

#[test]
fn checked_in_hardware_example_parses() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("hardware.example.toml");
    let result = load_hardware_test_config_from_path(&path);

    assert!(
        result.is_ok(),
        "failed to parse checked-in hardware example config: {result:?}"
    );

    let config = result.ok().unwrap_or_else(|| unreachable!("asserted config parse success"));
    assert_eq!(config.serial_device, "/dev/ttyACM0");
    assert_eq!(config.baud_rate, 115_200);
    assert_eq!(config.timeout_ms, 2_000);
    assert!(!config.expect_audio_bridge);
    assert!(!config.expect_trellis);
}