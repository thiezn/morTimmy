# TODO

## Protocol

- Add golden capture fixtures to lock host and firmware message compatibility.
- Define acknowledgement and retransmission semantics if the control link needs reliability above best-effort framing.
- Decide how protocol versioning should work once the wire schema starts changing rapidly.

## Firmware

- Replace the placeholder entrypoint with real Embassy executor tasks and scheduling.
- Implement USB CDC transport on top of the shared COBS framing layer.
- Add watchdog enforcement and safe startup behavior.
- Implement motor, servo, and ultrasonic drivers on real pins.
- Bring up Pico Audio Pack playback and queue or underrun telemetry.
- Bring up Trellis M4 keypad scanning, LED updates, and pad telemetry.
- Extend the current defmt bring-up logging into command, sensor, and fault telemetry once the real executor tasks exist.

## Host

- Implement the serial bridge around the shared framing layer.
- Add a real WebSocket API and telemetry fanout.
- Define remote-control arbitration and rate limiting.
- Flesh out the optional `nokhwa` camera abstraction for Linux and macOS.
- Extend `mortimmy-tools replay` to drive live hardware and capture outbound sessions.

## Tooling And Ops

- Add CI for `cargo check`, `cargo test`, `cargo clippy`, and `mdbook build docs`.
- Add Linux first-probe and BOOTSEL recovery notes beyond the current macOS + picotool README quickstart.
- Add cross-compilation guidance for Raspberry Pi deployment.
- Capture protocol fixtures from the new sample hardware-test config and live smoke runs.
- Add remote service install and health-check steps to `mortimmy-tools deploy host` once target host conventions are fixed.

## Documentation and schematics

- WireViz for wiring diagrams
- tsserial for custom PCB design (for instance, I'd like to have some kind of power board)


## Homekit support

the rust hap-rs crate is not really maintained anymore but might allow us to integate with homekit.

Maybe we can fork it, and transform it so we can use it with embassy as that does support async so the port would perhaps be doable? That would unlock rust in microcontrollers with no-std!!!

https://github.com/ewilken/hap-rs/issues/40

Either way, it would also be nice to have homekit support on the host side if thats the only feasible way at the moment. Its also perhaps less practical for my robot, although adding a flashlite to it would be cool, as well as having buttons on the robot that appears as switches is also cool.
