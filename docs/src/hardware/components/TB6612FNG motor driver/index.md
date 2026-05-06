# TB6612FNG Motor Driver

This is the wheel-motor driver used by the current PCB design.

## Overview

The Toshiba TB6612FNG is a dual brushed-DC motor driver with two MOSFET H-bridges. It can drive two bidirectional DC motors or one bipolar stepper motor and is a much better fit for 3.3 V microcontrollers than the L298N. In this robot, two TB6612FNG stages sit on the power distribution board: one stage for the left wheels and one stage for the right wheels.

## Key characteristics

- Dual H-bridge for 2 DC motors or 1 bipolar stepper
- Recommended motor supply `VM`: 4.5 V to 13.5 V
- Logic supply `VCC`: 2.7 V to 5.5 V
- Continuous output current: about 1.0 A to 1.2 A per channel depending on carrier PCB and thermal design
- Peak output current: up to about 3.0 A to 3.2 A per channel for short bursts
- PWM control input up to 100 kHz
- MOSFET output stage with much lower voltage drop than the L298N
- Standby control, coast mode, short-brake mode, thermal shutdown, undervoltage lockout, and internal flyback diodes

## Pinout

| Signal | Function |
| --- | --- |
| `VM` | Motor supply input |
| `VCC` | Logic supply input |
| `GND` | Shared logic and motor ground |
| `STBY` | Enables the H-bridges when held high |
| `PWMA` | PWM speed command for channel A |
| `AIN1` / `AIN2` | Direction and brake control for channel A |
| `A01` / `A02` | Motor A outputs |
| `PWMB` | PWM speed command for channel B |
| `BIN1` / `BIN2` | Direction and brake control for channel B |
| `B01` / `B02` | Motor B outputs |

## How it is used in this project

Two TB6612FNG stages are mounted on the PDB, one stage for the left wheels and one stage for the right wheels. The motion-controller Pico drives both stages over a 14-pin JST-XH control harness carrying 12 motor-control signals plus 2 grounds.

Left-side stage:

- `GP2` -> `PWMA`
- `GP3` -> `AIN1`
- `GP4` -> `AIN2`
- `GP5` -> `BIN1`
- `GP6` -> `BIN2`
- `GP7` -> `PWMB`

Right-side stage:

- `GP8` -> `PWMA`
- `GP9` -> `AIN1`
- `GP10` -> `AIN2`
- `GP11` -> `BIN1`
- `GP12` -> `BIN2`
- `GP13` -> `PWMB`

Both stages use the local `DRV_3V3` rail on the PDB for `VCC`. The `STBY` input is pulled high on the PDB with a local pull-up and exposed as a test pad or solder jumper rather than consuming another Pico GPIO. The `VM` input is the protected raw 2S motor rail `MOTOR_VM`.

## Electrical notes

- The TB6612FNG drops far less voltage than the L298N. A 2S pack that felt acceptable through the L298N can overdrive nominal 6 V motors through the TB6612FNG.
- Confirm the real stall current of each motor at the chosen `VM` and keep it comfortably below the per-channel limit.
- Place one 100 nF ceramic directly at `VCC`, one 100 nF ceramic directly at `VM`, and one 220 uF low-ESR bulk capacitor on the `MOTOR_VM` rail near each driver stage.
- Fit one 100 nF ceramic directly across each wheel-motor terminal pair at the motor body.
- Keep motor-current loops short and wide. Route logic traces away from the motor outputs and return the driver grounds into a low-impedance ground region.

## Mechanical notes

- Place each driver stage on the PDB close to its motor connectors and the incoming `MOTOR_VM` entry point.
- Give each stage stitched ground copper, short motor traces, and direct access to its local `220 uF + 100 nF` decoupling parts.

## References used

- Toshiba TB6612FNG datasheet
- SparkFun TB6612FNG Hookup Guide
- Pololu TB6612FNG carrier specifications
