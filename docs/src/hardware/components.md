# Components

This section collects the concrete hardware parts currently used or evaluated for the robot. Each component page keeps its own notes, pinouts, and related images in a dedicated folder so the documentation stays grouped by part.

## Compute and control

- [Raspberry Pi 3B V1.2](components/raspberry-pi-3b/index.md): main host computer for orchestration, transport, and higher-level coordination.
- [Pico Audio Pack](components/pi-audio-hat/index.md): audio add-on mounted on the Pico 2 W.

## Power

- [Power System](components/power/index.md): finalized 2S battery, protection, and rail-generation plan for the power distribution board.

## Sensors and user interface

- [HC-SR04 Ultrasonic Sensor](components/hc-sr04/index.md): forward distance sensing.
- Adafruit 4-channel I2C-safe Bi-directional Logic Level Converter (BSS138): shared 3.3 V to 5 V signal translation for HC-SR04 TRIG/ECHO lines and future open-drain peripherals.
- [Liquid Crystal Display](components/liquid-crystal-display/index.md): 16x2 character display for local status output.

## Actuation

- [TB6612FNG Motor Driver](components/TB6612FNG%20motor%20driver/index.md): dual MOSFET H-bridge used for the left and right wheel-drive stages on the PDB.
- [L298N Stepper / Motor Driver](components/l298n-stepper-motor-driver/index.md): earlier prototype driver board kept for historical reference.
- [Motors](components/motors/index.md): current DC motor models used for the prototype.
- [Servos](components/servos/index.md): pan / tilt servo interface and power plan for the control board.

## Notes

- The current robot wiring is documented in [Wiring](schematics/wiring.md).
- The hardware design rationale is documented in [Design](design.md).
