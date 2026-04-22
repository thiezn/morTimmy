# Components

This section collects the concrete hardware parts currently used or evaluated for the robot. Each component page keeps its own notes, pinouts, and related images in a dedicated folder so the documentation stays grouped by part.

## Compute and control

- [Raspberry Pi 3B V1.2](components/raspberry-pi-3b/index.md): main host computer for orchestration, transport, and higher-level coordination.
- [Pico Audio Pack](components/pi-audio-hat/index.md): audio add-on mounted on the Pico 2 W.

## Power

- [Power System](components/power/index.md): current battery and regulator arrangement for the prototype.

## Sensors and user interface

- [HC-SR04 Ultrasonic Sensor](components/hc-sr04/index.md): forward distance sensing.
- [Liquid Crystal Display](components/liquid-crystal-display/index.md): 16x2 character display for local status output.
- [Trellis M4 4x4](components/trellis-m4-4x4/index.md): keypad and LED matrix for future local interaction.

## Actuation

- [L298N Stepper / Motor Driver](components/l298n-stepper-motor-driver/index.md): dual H-bridge driver board used for wheel motors.

## Notes

- The current robot wiring is documented in [Wiring](schematics/wiring.md).
- The hardware design rationale is documented in [Design](design.md).
