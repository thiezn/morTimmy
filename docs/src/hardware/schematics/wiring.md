# Wiring schematics

The robot wiring diagram is generated from the WireViz source in `schematics/wiring/mortimmy.yml`.

The tables below distinguish between the pins that are already bound in firmware and the planned full hardware end state. The motion-controller motor, ultrasonic, and LCD mappings are implemented today; the servo and Audio Pack sections document the remaining next wiring target so the diagram and notes stay aligned.

## Motion controller wiring

The motion-controller firmware image targets the Pimoroni Pico LiPo 2 and binds the motor, ultrasonic, and LCD wiring directly in [firmware/rp2350/src/runtime/motion.rs](../../../firmware/rp2350/src/runtime/motion.rs). The generated diagram below should stay in sync with that firmware pin map.

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

The wiring plan uses two HC-SR04 modules mounted forward-left and forward-right at roughly 45 degrees. Both sensors share the single Adafruit 4-channel I2C-safe bi-directional logic level converter board. That breakout has four independent BSS138 channels with 10 kOhm pull-ups, plus shared `LV`, `HV`, and `GND` reference rails.

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

This is a better fit than the earlier passive divider note because the converter needs both the Pico `3V3` rail and the sensor `5V` rail to operate.

With conservative level shifting on both `TRIG` and `ECHO`, the two HC-SR04 modules fully consume the four BSS138 channels on the Adafruit board. There are no spare shifted channels left for a third ultrasonic sensor in this wiring plan.

The motion-controller firmware now binds both sensors directly:

- `GP14` / `GP15` for the forward-left sensor
- `GP16` / `GP17` for the forward-right sensor

The firmware polls the two sensors sequentially rather than simultaneously so one module finishes its acoustic burst before the other starts.

### LCD1602 wiring

Yes. After the motor controller claims `GP2` through `GP17` and the servo plan keeps `GP18` and `GP19` reserved, the Pico LiPo 2 still has exactly six free GPIO: `GP20`, `GP21`, `GP22`, `GP26`, `GP27`, and `GP28`. That is enough for the LCD in 4-bit write-only mode, so the display can live on the motion-controller Pico without colliding with the Audio Pack.

Main LCD data and control wiring:

| Pico LiPo 2 pin | LCD pin | Notes |
| --- | --- | --- |
| VBUS / 5V | VDD | LCD logic supply |
| GND | VSS | Common ground |
| GP20 | RS | Command or data select |
| GP21 | E | Enable strobe |
| GP22 | D4 | 4-bit data bus bit 0 |
| GP26 | D5 | 4-bit data bus bit 1 |
| GP27 | D6 | 4-bit data bus bit 2 |
| GP28 | D7 | 4-bit data bus bit 3 |

Supporting LCD wiring:

| Support path | LCD pin | Notes |
| --- | --- | --- |
| Contrast potentiometer wiper | VO | 10 kOhm trimmer between 5 V and ground |
| Ground | RW | Tied low for write-only mode |
| 5V through onboard or external resistor | LEDA | Backlight anode |
| Ground | LEDK | Backlight cathode |

This uses the last six free motion-controller GPIO after the servo reservation, so there is no extra GPIO headroom left on that Pico once the LCD is wired.

### Pan / tilt servo wiring

The desired full end state keeps the two hobby servos on the motion controller, but it does not reuse the 5 V logic rail. The hardware design already reserves a dedicated 6 V servo rail, so the Pico LiPo 2 only needs to provide PWM plus a shared reference ground.

Servo signal wiring:

| Pico LiPo 2 pin | Planned role | Servo lead | Notes |
| --- | --- | --- | --- |
| GP18 | PWM output | Pan servo signal | First free GPIO after the HC-SR04 pair; reserved for the pan axis |
| GP19 | PWM output | Tilt servo signal | Adjacent free GPIO reserved for the tilt axis |
| GND | Common ground | Ground on both servos | Must be common with the dedicated servo rail ground |

Servo power wiring:

| Power source | Servo lead | Notes |
| --- | --- | --- |
| Dedicated 6 V servo rail | V+ on both servos | Keep servo surge current off the 5 V logic rail |
| Shared ground | GND on both servos | Join the servo rail return and the Pico ground at the same reference |

The `board-motion-controller` firmware image does not bind `GP18` or `GP19` yet. These pins are reserved here as the planned pan / tilt end state so the already-working motor and ultrasonic wiring can remain stable while the servo runtime is added.

### motion-controller scope

The `board-motion-controller` firmware image owns L298N motor control, HC-SR04 ranging, and the local HD44780 LCD status display. Battery sensing is tracked separately and should not be wired back into the motion-controller capability set until the Pico LiPo 2 ADC path is verified. The `GP18` / `GP19` servo reservation above is the next hardware step, but those pins should stay electrically idle until the servo runtime is added.

## Audio controller wiring

The audio controller lives on the Pico 2 W and only drives the Pimoroni Pico Audio Pack.

### Pico Audio Pack wiring

The Pico Audio Pack is mechanically a full-board add-on, but it only consumes the Pico's I2S trio plus one optional mute / amp-enable control line.

| Pico 2 W pin | Audio Pack signal | Requirement | Notes |
| --- | --- | --- | --- |
| GP9 | DIN | Required | I2S audio data into the PCM5100A DAC |
| GP10 | BCK | Required | I2S bit clock |
| GP11 | LRCK / WS | Required | I2S word-select clock |
| GP29 | AMP_EN / MUTE | Optional but recommended | Used by Pimoroni examples and the driver scaffolding to mute or enable output |

The Pico only needs `GP9`, `GP10`, `GP11`, optional `GP29`, plus power and ground for the Audio Pack harness.

![mortimmy wiring diagram](./wiring/mortimmy.svg)
