# Desired Control

Use this pattern for motion, servo targets, and other continuously refreshed control domains.

## Shape

- One owner of desired state on the host or controller side.
- One owner of applied state on the firmware side.
- One idempotent apply method in firmware.
- One acknowledgement telemetry type.

## Why It Works

- Latest-wins semantics are simple to reason about.
- Reordering and retries are safer because the full state is carried every time.
- Mixed input sources such as keyboard, autonomy, and future websocket clients can all target the same snapshot.
- Firmware timeout behavior is easier to define because control freshness is tied to one message family.

## Mortimmy Example

- Host owns desired mode, drive, and servo.
- Firmware applies that through `ControlLoop::apply_desired_state`.
- `Ping`, audio chunks, and parameter updates remain one-shot commands.

## Common Mistakes

- Using `Stop` as a synonym for `drive = 0`.
- Splitting one control domain across several independent wire commands.
- Letting host-side clamping replace firmware-side safety checks.
- Turning patches or deltas into the default format before measuring payload pressure.
