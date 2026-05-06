# Servos

The control board reserves two hobby-servo outputs for pan and tilt motion.

## Electrical interface

- `GP18` is reserved for the pan servo PWM signal.
- `GP19` is reserved for the tilt servo PWM signal.
- Both servos are powered from the dedicated `SERVO_6V` rail generated on the PDB.
- Servo ground must remain common with the motion-controller ground.

## Board-level wiring

| Control-board signal | Servo lead | Notes |
| --- | --- | --- |
| `GP18` | Pan servo PWM | Reserved in hardware; firmware support is not bound yet |
| `GP19` | Tilt servo PWM | Reserved in hardware; firmware support is not bound yet |
| `SERVO_6V` | `V+` | Dedicated servo rail kept separate from `AUX_5V` |
| `GND` | `GND` | Shared reference with the Pico and the PDB |

## Connector plan

- Use JST-VH 3-pin connectors on the control board.
- Build adapter leads from JST-VH to the final servo plug style.
- Keep servo power wiring separate from the low-current LCD and sensor harnesses.

## Notes

- The hardware reserves these outputs now even though the `board-motion-controller` firmware does not drive them yet.
- Servo surge current stays on the dedicated `SERVO_6V` rail and should not be taken from the logic 5 V path.
