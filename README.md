# mortimmy

`mortimmy` is a Rust robotics workspace for a small rover split across two runtime environments:

- `firmware/rp2350` for deterministic embedded control on the Pimoroni Pico LiPo 2 (RP2350B)
- `host/mortimmy` for Raspberry Pi or macOS orchestration, transport bridging, telemetry, audio routing, and future camera control

The current hardware is:

- Raspberry Pi 3B V1.2
- Pimoroni Pico 2 W with Pico Audio Pack
- Pimoroni Pico LiPo 2
- Liquid Crystal Display LCM1602C V2.1
- HC-SR04 ultrasonic ranger
- Trellis M4 4x4 keypad and LED matrix

## Hardware

| Component | Version / Model | Purpose |
| --- | --- | --- |
| Raspberry Pi | 3B V1.2 | Main host computer running the daemon, USB device orchestration, telemetry, and higher-level coordination |
| Audio controller | Pico 2 W + Pico Audio Pack | USB-connected audio and display controller for playback and local visual feedback |
| Motion controller | Pimoroni Pico LiPo 2 (RP2350B) | USB-connected real-time controller for motors, sensors, and future servo logic |
| Audio add-on | Pimoroni Pico Audio Pack | I2S DAC and headphone / line-out hardware mounted on the Pico 2 W |
| Character display | LCM1602C V2.1 | 16x2 local status display driven in 4-bit mode from the audio Pico |
| Ultrasonic sensor | HC-SR04 | Forward distance measurement for obstacle awareness |
| Motor drivers | 2x L298N dual H-bridge | Drive four wheel motors from the motion controller GPIO |
| Wheel motors | 4x DC gear motors | Rover propulsion |
| Power regulator | UBEC / buck, 5 V 3 A | Regulates battery voltage down to the Raspberry Pi supply rail |
| Battery pack | 2x 18650, 2S | Main energy source for the rover |
| Keypad / LEDs | Trellis M4 4x4 | Future local input and status LED surface |

Shared crates under `crates/` define the core types, driver traits, and the wire protocol used across both sides. The protocol uses `postcard` for serialization and a COBS-framed transport with CRC16 integrity checks so the same message contract can move over USB CDC, UART, and recorded capture files.

Detailed architecture notes live in [docs/src/architecture/architecture.md](docs/src/architecture/architecture.md). Protocol-specific notes live in [docs/src/architecture/protocol.md](docs/src/architecture/protocol.md). Open follow-up work lives in [TODO.md](TODO.md).

## Current Scope

The workspace now contains a compileable control-plane scaffold with test and tooling support:

- `mortimmy-core` defines shared units, limits, modes, and error types.
- `mortimmy-protocol` defines the postcard schema, typed control and telemetry messages, CRC16 checks, and a stream-oriented COBS frame decoder sized for the current audio-forwarding contract.
- `mortimmy-drivers` defines hardware-facing traits for motors, servos, ultrasonic sensors, audio output, and Trellis input/LED control.
- `mortimmy-rp2350` is the active embedded crate for the RP2350B board/audio/Trellis direction using `embassy-rp`, `embassy-usb`, and `panic-probe`, and it now applies shared protocol commands directly into scaffold state with matching telemetry snapshots.
- `mortimmy` provides `start` and `config` subcommands backed by a nested `config.toml` layout for serial, websocket, telemetry, audio, camera, and logging settings, and the `start` path now runs a keyboard-driven brain loop against either the loopback Pico scaffold or future live transports.
- `mortimmy-tools` can inspect and replay captured framed protocol streams, build host artifacts, and deploy RP2350 firmware through BOOTSEL or probe-rs workflows.
- `mortimmy-integration-test` provides the root integration harness for protocol roundtrips and future live hardware smoke tests.

## Workspace Layout

```text
.
├── Cargo.toml
├── README.md
├── TODO.md
├── crates/
│   ├── core/
│   ├── drivers/
│   └── protocol/
├── docs/
│   └── src/
│       └── architecture/
├── firmware/
│   └── rp2350/
├── host/
│   ├── mortimmy/
│   └── tools/
├── integration_test/
└── scripts/
```

## Prerequisites

- Rust toolchain from [rust-toolchain.toml](rust-toolchain.toml)
- `cargo`
- For docs: `mdbook` and `mdbook-mermaid`
- For embedded flashing and debugging: `cargo install probe-rs-tools --locked`
- For UF2 generation and BOOTSEL uploads: `cargo install elf2uf2-rs --locked`
- For BOOTSEL uploads over the RP2350 picoboot interface on macOS: `brew install picotool`

The pinned toolchain includes `thumbv8m.main-none-eabihf`, so a fresh `rustup` install is enough to build the active embedded target.

During development, you can invoke the deployment CLI directly from the workspace with `cargo run -p mortimmy-tools -- ...`. If you want a standalone binary on your `PATH`, install it from this checkout with `cargo install --path host/tools`.

## Build, Test, And Lint

Build the whole workspace:

```bash
cargo check --workspace
```

Validate the active embedded target specifically:

```bash
cargo check -p mortimmy-rp2350 --target thumbv8m.main-none-eabihf
```

Build the firmware ELF that is used by both probe-rs and UF2 workflows:

```bash
cargo build -p mortimmy-rp2350 --target thumbv8m.main-none-eabihf
```

Run tests and lints:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

Build the documentation site:

```bash
mdbook build docs
```

Optional coverage report:

```bash
./scripts/coverage.sh
./scripts/coverage.sh --html
```

## Firmware Bring-Up On macOS

The current RP2350 firmware can be smoke-tested without any attached peripherals. On the embedded target it now starts an Embassy USB CDC task that decodes the shared framed protocol, applies commands through the firmware scaffold, and writes telemetry responses back over the same USB link.

The shared protocol is now exercised inside the firmware crate as well: desired-state, parameter, audio, Trellis LED, status, and ping commands are applied to the firmware scaffold under unit tests, and the default audio chunk size is aligned across host planning, wire payload sizing, and firmware buffering.

Validate the firmware locally before touching hardware:

```bash
cargo test -p mortimmy-rp2350
cargo run -p mortimmy-tools -- deploy firmware uf2
```

Upload over plain USB with the Pico in BOOTSEL mode:

1. Hold the board's `BOOTSEL` button while connecting USB-C.
2. Wait for either a BOOTSEL volume such as `RP2350` or `RPI-RP2` to appear, or for `picotool info` to list an RP2350 BOOTSEL device.
3. Run:

```bash
cargo run -p mortimmy-tools -- deploy firmware uf2-deploy
```

If `picotool` is installed, `mortimmy-tools deploy firmware uf2-deploy` prefers `picotool load -v -x -t uf2` and reboots the board directly. This is the recommended macOS path because mass-storage copies to the BOOTSEL volume have hung behind `storagekitd` on this machine. The tool falls back to copying `NEW.UF2` only when `picotool` cannot access the BOOTSEL interface.

This flow has been validated end to end on this machine. A successful BOOTSEL deploy reported `Loading into Flash ... 100%`, `Verifying Flash ... 100%`, `OK`, and `The device was rebooted to start the application.`

If the deploy command reports `Unable to find a board in BOOTSEL mode`, the board is not in BOOTSEL mode yet or neither the picoboot interface nor the BOOTSEL volume is visible to macOS. The tool now prints the exact BOOTSEL entry steps before exiting.

Flash and stream RTT logs through an SWD probe:

```bash
cargo run -p mortimmy-tools -- deploy firmware probe-list
cargo run -p mortimmy-tools -- deploy firmware flash --probe-index 0
cargo run -p mortimmy-tools -- deploy firmware run --probe-index 0
```

The probe-based workflow uses the `probe-rs` chip name `RP235x`. A successful RTT session starts with a line like:

```text
boot board=Pimoroni Pico LiPo 2 mcu=RP2350B flash=16777216 psram=8388608 transport=usb-cdc mode=teleop audio=host-waveform-bridge chunk_samples=240 trellis=false ultrasonic=false battery=false
```

If `firmware-flash` reports `No debug probes were found`, attach an SWD-compatible debug probe or fall back to the BOOTSEL/UF2 path above.

On the validation machine used for the BOOTSEL bring-up, `mortimmy-tools deploy firmware probe-list` currently reports `No debug probes were found.`, so BOOTSEL remains the only available hardware flashing path.

After a successful BOOTSEL upload, the Pico should enumerate as a runtime USB CDC device such as `/dev/cu.usbmodem*` on macOS. That runtime enumeration path is implemented in firmware; regular live smoke coverage for the full host-to-device roundtrip remains tracked in `TODO.md`.

## Run Locally On macOS

The host brain now supports both the `loopback` transport for local proofing and the `serial` transport for a live Pico USB CDC device. Both paths use the same routing, postcard codec, and CRC16 plus COBS framing.

`teleop` with zero drive is now the nominal stopped state. If the firmware link times out, the controller enters `fault`, resets to its safe failed state, and the host reasserts its last requested `teleop` or `autonomous` mode after reconnect.

Create or update a config file:

```bash
cargo run -p mortimmy -- config --config ./config.toml --print
```

Run the brain loop with keyboard input and the in-process Pico scaffold:

```bash
cargo run -p mortimmy -- start \
  --config ./tmp/brain-loopback.toml \
  --input-backend keyboard \
  --transport-backend loopback
```

Useful keyboard commands during bring-up:

```text
p | ping
x | stop
w | forward
s | reverse
a | left
d | right
t | teleop mode
u | autonomous mode (default servo-scan plan)
f | fault mode
q | quit
```

Switch the local keyboard to tank controls when you want per-tread input:

```bash
cargo run -p mortimmy -- start \
  --config ./tmp/brain-loopback.toml \
  --input-backend keyboard \
  --keyboard-drive-style tank \
  --transport-backend loopback
```

You can also press `m` while the session is running to toggle between `wasd` and `tank` driving. In tank mode, `w` and `s` control the left tread, `e` and `d` control the right tread, and `a` gives a left pivot shortcut by driving the left tread forward while reversing the right tread. To accept input from only one controller instead of letting the most recent controller win, pass `--controller-lock KIND:INSTANCE`, for example `--controller-lock keyboard:local`.

Websocket controllers are now live on the configured bind address as well. Each websocket client appears as its own `websocket:client-N` controller, so `--controller-lock websocket:client-1` will pin the brain to the first connected websocket client. Send JSON text frames such as:

```json
{"type":"control","drive":{"forward":1.0,"turn":0.0,"speed":300}}
{"type":"control","drive":null}
{"type":"command","command":"ping"}
{"type":"command","command":"teleop"}
```

`forward` and `turn` are normalized from `-1.0` to `1.0` and map onto the existing desired-state control path. Supported websocket commands are `ping`, `stop`, `teleop`, `autonomous`, `fault`, and `quit`.

This path has been validated on this machine with `p` followed by `q`; the host logged startup, encoded the ping command over the shared protocol, and received `telemetry pong: Pong` back from the firmware scaffold.

Run the same brain loop against a flashed Pico over USB CDC:

```bash
cargo run -p mortimmy -- start \
  --config ./tmp/brain-serial.toml \
  --input-backend keyboard \
  --transport-backend serial \
  --serial-device /dev/cu.usbmodem0001
```

For live hardware validation, `p` should return `telemetry pong: Pong`, holding `w` plus `a` should produce one combined desired-state control path, and `u` should switch the host into the built-in autonomous servo-scan plan until another mode is selected.

If you want to exercise the future camera seam, enable the optional `nokhwa` backend:

```bash
cargo run -p mortimmy --features camera-nokhwa -- start \
  --config ./config.toml \
  --serial-device /dev/tty.usbmodem0001 \
  --camera-enabled=true \
  --camera-backend nokhwa
```

Inspect or replay a captured protocol stream:

```bash
cargo run -p mortimmy-tools -- inspect ./captures/session.bin
cargo run -p mortimmy-tools -- replay ./captures/session.bin --dry-run
```

## Run On Raspberry Pi

The host daemon is intended to run directly on Raspberry Pi OS once the USB/serial bridge is wired to the firmware.

Build on the Pi:

```bash
cargo build -p mortimmy --release
```

Write or update the Pi config:

```bash
./target/release/mortimmy config --config ./config.toml \
  --serial-device /dev/ttyACM0 \
  --websocket-bind 0.0.0.0:9001 \
  --audio-enabled=true \
  --audio-backend firmware-bridge
```

Start the daemon:

```bash
./target/release/mortimmy start --config ./config.toml
```

Example config layout:

```toml
[serial]
device_path = "/dev/ttyACM0"
baud_rate = 115200

[websocket]
bind_address = "0.0.0.0:9001"

[telemetry]
publish_interval_ms = 100
queue_capacity = 256

[audio]
enabled = true
backend = "firmware-bridge"
sample_rate_hz = 24000
channels = 1
chunk_samples = 240
volume_percent = 100

[camera]
enabled = false
backend = "disabled"
device_index = 0
width = 640
height = 480
fps = 30

[logging]
level = "info"
no_color = false
```

## Integration Tests

The `integration_test` crate contains both portable protocol tests and ignored live-hardware smoke tests. Hardware tests are configured through the `MORTIMMY_HW_CONFIG` environment variable.

A checked-in bare-board sample lives at [integration_test/hardware.example.toml](integration_test/hardware.example.toml). It assumes no audio bridge or Trellis hardware is attached yet.

Example hardware test config:

```toml
serial_device = "/dev/ttyACM0"
baud_rate = 115200
timeout_ms = 2000
expect_audio_bridge = false
expect_trellis = false
```

Run the ignored live tests:

```bash
MORTIMMY_HW_CONFIG=./integration_test/hardware.example.toml cargo test -p mortimmy-integration-test -- --ignored
```

## Deployment CLI

The deployment surface now lives in `mortimmy-tools deploy`. The examples below use the installed binary name; if you are running straight from the workspace, prefix them with `cargo run -p mortimmy-tools --` instead.

Host workflows:

```bash
mortimmy-tools deploy host build
mortimmy-tools deploy host print-artifact
mortimmy-tools deploy host local-install --sudo
mortimmy-tools deploy host remote-copy --remote-host pi@raspberrypi.local
mortimmy-tools deploy host remote-install --remote-host pi@raspberrypi.local --sudo
```

Firmware workflows:

```bash
mortimmy-tools deploy firmware build
mortimmy-tools deploy firmware print-artifact
mortimmy-tools deploy firmware uf2
mortimmy-tools deploy firmware uf2-deploy
mortimmy-tools deploy firmware probe-list
mortimmy-tools deploy firmware flash --probe-index 0
mortimmy-tools deploy firmware run --probe-index 0
```

The most useful customization flags are:

- Global logging: `--log-level trace|debug|info|warn|error`, `--no-color`
- Host deployment: `--package`, `--bin`, `--profile`, `--install-dir`, `--remote-host`, `--remote-dir`
- Firmware build and UF2 packaging: `--target rp2350`, `--profile`, `--output`
- Firmware probe workflows: `--probe-index`, `--protocol swd|jtag`, `--speed-khz`
- Firmware flashing behavior: `--chip-erase`, `--preverify`, `--verify`, `--restore-unwritten`, `--disable-double-buffering`

`mortimmy-tools deploy firmware uf2-deploy` prefers `picotool` when it can see the board in BOOTSEL mode and falls back to the mounted UF2 volume copy path only when the picoboot interface is unavailable. `mortimmy-tools deploy firmware run` currently delegates the monitor UX to `probe-rs run`, which preserves the existing defmt/RTT workflow while keeping the build and target selection inside the Rust deployment tool.

## Documentation

- Architecture: [docs/src/architecture/architecture.md](docs/src/architecture/architecture.md)
- Roadmap: [TODO.md](TODO.md)
- MdBook summary: [docs/src/SUMMARY.md](docs/src/SUMMARY.md)
