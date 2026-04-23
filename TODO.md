# TODO

- Physically connect the L298N control wiring, HC-SR04 wiring, and power connections for the motor/sensor Pico, then run live motion and ultrasonic smoke tests against the flashed motion-controller image.
- Integrate the audio-controller firmware image with the real board wiring so the Pico Audio Pack output and LCD1602 display are exercised through the runtime instead of scaffold state only.
- Validate the audio-controller firmware image on hardware and verify the host sees `AudioController` plus the expected capability bits over the live serial link.
- Verify the Pico LiPo 2 battery-sense net on the actual board or schematic and confirm whether `GP29` is really the intended measurement point.
- Determine how to read that battery-sense net on the RP235x target with `embassy-rp`, or choose a lower-level ADC path if the HAL does not expose the pin.
- Implement battery telemetry and only re-enable the motion-controller battery-monitor capability after the ADC path is verified on hardware.
