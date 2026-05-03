# Wiring schematics

The current robot wiring diagram is generated from the WireViz source in `schematics/wiring/mortimmy.yml`.

## Motion controller wiring

The current motion-controller firmware image targets the Pimoroni Pico LiPo 2 and binds the motor and ultrasonic wiring directly in [firmware/rp2350/src/usb.rs](../../../firmware/rp2350/src/usb.rs). The generated diagram below should stay in sync with that firmware pin map.

### L298N control wiring

Left motor-driver board:

| Pico LiPo 2 pin | Firmware role | L298N pin | Notes |
| --- | --- | --- | --- |
| GP2 | PWM slice 1 A | ENA | Front-left channel enable |
| GP3 | GPIO output | IN1 | Front-left direction A |
| GP4 | GPIO output | IN2 | Front-left direction B |
| GP5 | GPIO output | IN3 | Rear-left direction A |
| GP6 | GPIO output | IN4 | Rear-left direction B |
| GP7 | PWM slice 3 B | ENB | Rear-left channel enable |
| GND | Common ground | GND | Required reference between Pico and driver |

Right motor-driver board:

| Pico LiPo 2 pin | Firmware role | L298N pin | Notes |
| --- | --- | --- | --- |
| GP8 | PWM slice 4 A | ENA | Front-right channel enable |
| GP9 | GPIO output | IN1 | Front-right direction A |
| GP10 | GPIO output | IN2 | Front-right direction B |
| GP11 | GPIO output | IN3 | Rear-right direction A |
| GP12 | GPIO output | IN4 | Rear-right direction B |
| GP13 | PWM slice 6 B | ENB | Rear-right channel enable |
| GND | Common ground | GND | Required reference between Pico and driver |

Motor outputs:

| Driver board | Output pair | Motor |
| --- | --- | --- |
| Left L298N | MotorA +/- | Front-left motor |
| Left L298N | MotorB +/- | Rear-left motor |
| Right L298N | MotorA +/- | Front-right motor |
| Right L298N | MotorB +/- | Rear-right motor |

### HC-SR04 wiring

The current wiring plan uses two HC-SR04 modules mounted forward-left and forward-right at roughly 45 degrees. Both sensors share the single Adafruit 4-channel I2C-safe bi-directional logic level converter board. That breakout has four independent BSS138 channels with 10 kOhm pull-ups, plus shared `LV`, `HV`, and `GND` reference rails.

Shared rails:

| Pico LiPo 2 pin | Level converter pin | HC-SR04 pin | Notes |
| --- | --- | --- | --- |
| 3V3 OUT (The + pin on our Pico) | LV | - | Required low-side reference for the BSS138 board |
| VBUS / 5V | HV | VCC on both sensors | High-side reference rail and shared sensor supply |
| GND | GND | GND on both sensors | Shared logic ground across Pico, converter, and both sensors |

Forward-left sensor:

| Pico LiPo 2 pin | Level converter pin | HC-SR04 pin | Notes |
| --- | --- | --- | --- |
| GP14 | LV1 | TRIG via HV1 | Forward-left trigger output |
| GP15 | LV2 via HV2 | ECHO | Forward-left echo input level shifted back to 3.3 V |

Forward-right sensor:

| Pico LiPo 2 pin | Level converter pin | HC-SR04 pin | Notes |
| --- | --- | --- | --- |
| GP16 | LV3 | TRIG via HV3 | Forward-right trigger output |
| GP17 | LV4 via HV4 | ECHO | Forward-right echo input level shifted back to 3.3 V |

This is a better fit than the earlier passive divider note because the converter needs both the Pico `3V3` rail and the sensor `5V` rail to operate. The current WireViz source now models that explicitly.

With conservative level shifting on both `TRIG` and `ECHO`, the two HC-SR04 modules fully consume the four BSS138 channels on the Adafruit board. There are no spare shifted channels left for a third ultrasonic sensor in this wiring plan.

The motion-controller firmware now binds both sensors directly:

- `GP14` / `GP15` for the forward-left sensor
- `GP16` / `GP17` for the forward-right sensor

The firmware polls the two sensors sequentially rather than simultaneously so one module finishes its acoustic burst before the other starts.

### Current motion-controller scope

The `board-motion-controller` firmware image is intentionally scoped to L298N motor control plus HC-SR04 ranging. Battery sensing is tracked separately and should not be wired back into the motion-controller capability set until the Pico LiPo 2 ADC path is verified.

![mortimmy wiring diagram](./wiring/mortimmy.svg)
