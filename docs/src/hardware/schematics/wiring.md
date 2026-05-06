# Wiring schematics

The robot wiring diagram is generated from the WireViz source in `schematics/wiring/mortimmy.yml`.

The tables below separate the firmware-backed motion-controller wiring from the reserved servo pins that are not bound in firmware yet. The TB6612FNG motor allocation, ultrasonic pair, and LCD mappings are already fixed in the current design.

## Motion controller wiring

The motion-controller firmware image targets the Pimoroni Pico LiPo 2 and binds the motor, ultrasonic, and LCD wiring directly in [firmware/rp2350/src/runtime/motion.rs](../../../firmware/rp2350/src/runtime/motion.rs). The current code still names the motor backend after the older L298N driver, but the active GPIO allocation already matches one TB6612FNG stage per side on the PDB. The Pico reaches those driver stages through the 14-pin control harness described in [PCB designs](pcb.md), and the generated diagram below should stay in sync with that firmware pin map.

### Pico 2 power and USB link

The PCB design uses a Raspberry Pi Pico 2 module on the control board. Power it from the board `AUX_5V` rail into the Pico `VSYS` pin through a reverse-blocking ideal-diode stage. Do not feed the Pico from the control-board `3V3` rail, and do not tie `AUX_5V` directly to the Pico `VBUS` pin.

Keep the Raspberry Pi 3B connection as a native USB device link on the Pico USB connector:

- USB `D+` and `D-` carry the actual Pi to Pico communication.
- USB `GND` and shield remain bonded through the cable.
- Host `VBUS` from the Pi USB port still reaches the Pico USB connector so the Pico sees USB attach and supports normal programming.
- The reverse-blocking `AUX_5V` to `VSYS` path prevents that host `VBUS` from backfeeding the board `AUX_5V` rail.

No extra UART is required for normal Pi 3B communication or programming. Add SWD only if you want board-level debug access.

### TB6612FNG control wiring

Per side, the current firmware still uses six motor-control signals: two PWM outputs and four direction pins. That maps directly onto `PWMA`, `AIN1`, `AIN2`, `BIN1`, `BIN2`, and `PWMB`, so the TB6612FNG interface does not need a new Pico GPIO because `STBY` is pulled high locally on the PDB.

Left motor-driver stage:

| Pico LiPo 2 pin | Firmware role | TB6612FNG pin | Notes |
| --- | --- | --- | --- |
| GP2 | PWM slice 1 A | PWMA | Front-left speed command |
| GP3 | GPIO output | AIN1 | Front-left direction A |
| GP4 | GPIO output | AIN2 | Front-left direction B |
| GP5 | GPIO output | BIN1 | Rear-left direction A |
| GP6 | GPIO output | BIN2 | Rear-left direction B |
| GP7 | PWM slice 3 B | PWMB | Rear-left speed command |
| GND | Common ground | GND | Required reference between Pico and driver |

Right motor-driver stage:

| Pico LiPo 2 pin | Firmware role | TB6612FNG pin | Notes |
| --- | --- | --- | --- |
| GP8 | PWM slice 4 A | PWMA | Front-right speed command |
| GP9 | GPIO output | AIN1 | Front-right direction A |
| GP10 | GPIO output | AIN2 | Front-right direction B |
| GP11 | GPIO output | BIN1 | Rear-right direction A |
| GP12 | GPIO output | BIN2 | Rear-right direction B |
| GP13 | PWM slice 6 B | PWMB | Rear-right speed command |
| GND | Common ground | GND | Required reference between Pico and driver |

These 12 control signals plus two ground references form the 14-pin board-to-board harness between the control board and the PDB.

Local support wiring:

| Support net | TB6612FNG pin | Notes |
| --- | --- | --- |
| PDB local `DRV_3V3` rail | VCC | Local motor-driver logic supply on the PDB |
| `DRV_3V3` via local pull-up on the PDB | STBY | Hold high to enable the bridges; expose a test pad or solder jumper on the PDB for bring-up and fault isolation |
| Motor rail | VM | Feed from the dedicated motor power path; do not assume raw 2S is safe for nominal 6 V motors |
| GND | GND | Shared reference between Pico, driver stage, and motor power return |

Motor outputs:

| Driver stage | Output pair | Motor |
| --- | --- | --- |
| Left TB6612FNG | A01 / A02 | Front-left motor |
| Left TB6612FNG | B01 / B02 | Rear-left motor |
| Right TB6612FNG | A01 / A02 | Front-right motor |
| Right TB6612FNG | B01 / B02 | Rear-right motor |

### HC-SR04 wiring

The wiring plan uses two HC-SR04 modules mounted forward-left and forward-right at roughly 45 degrees. Both sensors share the single Adafruit 4-channel I2C-safe bi-directional logic level converter board. That breakout has four independent BSS138 channels with 10 kOhm pull-ups, plus shared `LV`, `HV`, and `GND` reference rails.

Shared rails:

| Control-board source | Level converter pin | HC-SR04 pin | Notes |
| --- | --- | --- | --- |
| 3V3 OUT (The + pin on our Pico) | LV | - | Required low-side reference for the BSS138 board |
| `5V_FILT` | HV | VCC on both sensors | Filtered 5 V rail for the HC-SR04 pair |
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

Because the TB6612FNG `STBY` signal is handled locally on the PDB rather than by an extra GPIO, the motor section still claims only `GP2` through `GP17` and the servo plan keeps `GP18` and `GP19` reserved. The Pico LiPo 2 still has exactly six free GPIO: `GP20`, `GP21`, `GP22`, `GP26`, `GP27`, and `GP28`. That is enough for the LCD in 4-bit write-only mode, so the display can live on the motion-controller Pico without colliding with the Audio Pack.

Main LCD data and control wiring:

| Control-board source / Pico GPIO | LCD pin | Notes |
| --- | --- | --- |
| `5V_FILT` | VDD | Filtered 5 V logic supply |
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
| `5V_FILT` through onboard or external resistor | LEDA | Backlight anode |
| Ground | LEDK | Backlight cathode |

This uses the last six free motion-controller GPIO after the servo reservation, so there is no extra GPIO headroom left on that Pico once the LCD is wired.

### Pan / tilt servo wiring

The desired full end state keeps the two hobby servos on the motion controller, but it does not reuse the 5 V logic rail. The hardware design already reserves a dedicated 6 V servo rail, so the Pico LiPo 2 only needs to provide PWM plus a shared reference ground.

Servo signal wiring:

| Pico LiPo 2 pin | Reserved role | Servo lead | Notes |
| --- | --- | --- | --- |
| GP18 | PWM output | Pan servo signal | First free GPIO after the HC-SR04 pair; reserved for the pan axis |
| GP19 | PWM output | Tilt servo signal | Adjacent free GPIO reserved for the tilt axis |
| GND | Common ground | Ground on both servos | Must be common with the dedicated servo rail ground |

Servo power wiring:

| Power source | Servo lead | Notes |
| --- | --- | --- |
| Dedicated 6 V servo rail | V+ on both servos | Keep servo surge current off the 5 V logic rail |
| Shared ground | GND on both servos | Join the servo rail return and the Pico ground at the same reference |

The `board-motion-controller` firmware image does not bind `GP18` or `GP19` yet. These pins remain reserved for the chosen pan / tilt hardware end state so the already-working motor and ultrasonic wiring can remain stable while the servo runtime is added.

### motion-controller scope

The `board-motion-controller` firmware image owns wheel-motor control through the TB6612-compatible `GP2` through `GP13` interface, HC-SR04 ranging, and the local HD44780 LCD status display. Battery sensing is tracked separately and should not be wired back into the motion-controller capability set until the Pico LiPo 2 ADC path is verified. The `GP18` / `GP19` servo reservation above is the next hardware step, but those pins should stay electrically idle until the servo runtime is added.

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
