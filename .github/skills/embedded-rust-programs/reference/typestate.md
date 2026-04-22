# Typestate

Typestate encodes state in types so invalid transitions fail at compile time.

## Good Uses

- Peripheral initialization order.
- Builder-style setup that must consume one state to create another.
- Capability gating such as `ConfiguredUart`, `EnabledPwm`, or `UsbReady`.
- APIs where runtime misuse would be expensive, dangerous, or impossible to recover from.

## Bad Uses

- Live robot modes such as `Idle`, `Teleop`, `Autonomous`, or `Fault`.
- Autonomy plans that change because of timers, sensors, reconnects, or operator input.
- Control loops that must react to runtime telemetry.

## Rule Of Thumb

If a state changes because the outside world changes, prefer a runtime state machine.

This follows the embedded Rust book guidance: typestate is strongest when it enforces design contracts and configuration sequences, not when it tries to freeze dynamic behavior into compile-time types.