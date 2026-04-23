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

| Pico LiPo 2 pin | HC-SR04 pin | Notes |
| --- | --- | --- |
| VBUS / 5V | VCC | Sensor is powered from the Pico USB 5V rail in the current wiring plan |
| GP14 | TRIG | Trigger output driven directly by firmware |
| GP15 | ECHO | Route through the ECHO divider before the Pico pin |
| GND | GND | Shared logic ground |

The ECHO line must be level shifted down to 3.3 V before it reaches GP15. The current WireViz source models that as `ECHO_DIV1` between the raw sensor ECHO output and the Pico input.

### Current motion-controller scope

The `board-motion-controller` firmware image is intentionally scoped to L298N motor control plus HC-SR04 ranging. Battery sensing is tracked separately and should not be wired back into the motion-controller capability set until the Pico LiPo 2 ADC path is verified.

![mortimmy wiring diagram](./wiring/mortimmy.svg)
