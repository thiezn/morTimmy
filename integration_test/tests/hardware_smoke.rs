use mortimmy_integration_test::load_hardware_test_config;

#[test]
fn live_hardware_config_is_optional() {
    let _ = load_hardware_test_config().unwrap();
}

#[test]
#[ignore = "requires live connected hardware and firmware support"]
fn usb_link_ping_pong_smoke() {
    let Some(_config) = load_hardware_test_config().unwrap() else {
        return;
    };

    // Future work: open the configured serial device, send Ping, and assert Pong.
}

#[test]
#[ignore = "requires live audio-capable hardware"]
fn audio_bridge_accepts_pcm_chunks() {
    let Some(_config) = load_hardware_test_config().unwrap() else {
        return;
    };

    // Future work: stream a deterministic PCM chunk sequence and assert firmware status telemetry.
}

#[test]
#[ignore = "requires live Trellis hardware"]
fn trellis_led_mask_updates_without_manual_input() {
    let Some(_config) = load_hardware_test_config().unwrap() else {
        return;
    };

    // Future work: command a LED mask sweep and validate device-side acknowledgement/telemetry.
}
