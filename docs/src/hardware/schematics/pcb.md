# PCB designs

The hardware is split into two boards:

- A Power Distribution Board (PDB) that handles battery input protection, rail generation, the two TB6612FNG stages, wheel-motor outputs, indicators, and test points.
- A control board that carries the Pico 2, the HC-SR04 level shifting, the LCD and servo connectors, and the low-current control harness back to the PDB.

## Fixed design choices

- The wheel motors run from protected raw 2S battery voltage on `MOTOR_VM`. There is no motor buck stage and no motor-source selection jumper.
- The two TB6612FNG stages live on the PDB and are driven from the control board through a single 14-pin JST-XH control harness.
- The PDB generates `AUX_5V` for logic-side 5 V loads, `SERVO_6V` for both servos, and a local-only `DRV_3V3` rail for the TB6612FNG `VCC` pins and the shared `STBY` pull-up.
- The control board powers the Pico 2 from `AUX_5V` into `VSYS` through a reverse-blocking ideal-diode power path. Do not tie `AUX_5V` directly into the Pico `VBUS` pin.
- The control board derives two local 5 V branches from `AUX_5V`: `PICO_VSYS` for the Pico 2 and `5V_FILT` for the LCD and HC-SR04 connectors. The Pico-generated `3V3` rail remains the only control-board 3.3 V rail.
- The Raspberry Pi 3B keeps its link to the motion-controller Pico over the Pico USB connector. USB `D+`, `D-`, `GND`, `SHELL`, and host `VBUS` stay on the USB cable for device attach and programming, but `VBUS` is not the primary board power path.
- Use `0603` as the default footprint for small passives: resistors, indicator LEDs, pull-ups, 100 nF ceramics, and the 5 V ferrite bead.
- Use JST-VH for battery, board-to-board power, motor, and servo connectors.
- Use JST-XH for the HC-SR04 and LCD harnesses.

## Board architecture

- Keep all battery, buck-converter, servo, and motor-current loops on the PDB.
- Use only two interconnects between boards: one 4-pin JST-VH power harness and one 14-pin JST-XH driver-control harness.
- Keep the control board free of raw battery and motor current so the Pico, ultrasonic inputs, and LCD bus stay on the quieter board.
- Place the TB6612FNG stages beside the wheel-motor connectors so each high-current loop remains short.

## PCB defaults

- Use 2-layer FR-4 for both boards.
- Use 2 oz copper on the PDB. Use 1 oz copper on the control board.
- Keep all battery, motor, and servo traces wide and short. Keep the TB6612FNG stages on the PDB edge nearest the motor connectors.
- Use a continuous ground reference on both boards, but keep motor return current out of the sensor and LCD area.
- Add ground stitching vias anywhere the high-current ground path changes layer, around each TB6612FNG ground region, and beside the board-to-board ground pins.

## Power Distribution Board

### High-level overview

- Route `J_BAT` through the blade fuse, the master switch, the reverse-polarity MOSFET stage, and the battery-side TVS clamp. Treat the protected output as `VBAT_PROT`.
- Feed `MOTOR_VM` directly from `VBAT_PROT`. Place a 470 uF low-ESR capacitor at the protected battery entry and one `220 uF + 100 nF` decoupling set at each TB6612FNG stage.
- Generate one 5.1 V buck output and split it into an `AUX_5V` branch for the control board and a protected `PI_5V` branch for the Raspberry Pi connector.
- Put a resettable fuse or current-limited load switch in series with `PI_5V`, and place at least 100 uF bulk capacitance directly beside `J_PI_PWR`.
- Generate `SERVO_6V` with its own buck converter and keep the servo rail separate from `AUX_5V` all the way to the control-board harness.
- Generate a local `DRV_3V3` rail on the PDB only. Use it for both TB6612FNG `VCC` pins and the shared `STBY` pull-up; do not export this rail off-board.
- Fit one 100 nF ceramic directly across each wheel-motor terminal pair at the motor body.
- Add LED indicators and test points for `VBAT`, `VBAT_PROT`, `MOTOR_VM`, `AUX_5V`, `PI_5V`, and `SERVO_6V`, plus test access to `DRV_3V3` and `STBY`.

### PDB connector plan

| Ref | Connector | Family | Pinout | Notes |
| --- | --- | --- | --- | --- |
| `J_BAT` | Battery input | JST-VH 2-pin | `VBAT`, `GND` | Battery holder to PDB |
| `J_CTRL_AUX` | Control-board logic and servo feed | JST-VH 4-pin | `AUX_5V`, `SERVO_6V`, `GND`, `GND` | `AUX_5V` stays separate from `SERVO_6V`; local 5 V filtering happens on the control board |
| `J_CTRL_DRV` | Control-board driver control | JST-XH 14-pin | See motor-control harness pin map below | Single low-current harness for both TB6612FNG stages |
| `J_PI_PWR` | Raspberry Pi power | JST-VH 2-pin | `PI_5V`, `GND` | Dedicated Pi feed with its own current limiting |
| `J_MOTOR_FL` | Front-left motor | JST-VH 2-pin | `A01`, `A02` | Front-left wheel motor output |
| `J_MOTOR_RL` | Rear-left motor | JST-VH 2-pin | `B01`, `B02` | Rear-left wheel motor output |
| `J_MOTOR_FR` | Front-right motor | JST-VH 2-pin | `A01`, `A02` | Front-right wheel motor output |
| `J_MOTOR_RR` | Rear-right motor | JST-VH 2-pin | `B01`, `B02` | Rear-right wheel motor output |

### Motor-control harness pin map

| Pin | Net | Pico 2 GPIO | PDB destination | Notes |
| --- | --- | --- | --- | --- |
| 1 | `GND` | `GND` | Ground reference | Left-stage return and harness shield point |
| 2 | `L_PWMA` | `GP2` | Left TB6612FNG `PWMA` | Front-left speed command |
| 3 | `L_AIN1` | `GP3` | Left TB6612FNG `AIN1` | Front-left direction A |
| 4 | `L_AIN2` | `GP4` | Left TB6612FNG `AIN2` | Front-left direction B |
| 5 | `L_BIN1` | `GP5` | Left TB6612FNG `BIN1` | Rear-left direction A |
| 6 | `L_BIN2` | `GP6` | Left TB6612FNG `BIN2` | Rear-left direction B |
| 7 | `L_PWMB` | `GP7` | Left TB6612FNG `PWMB` | Rear-left speed command |
| 8 | `GND` | `GND` | Ground reference | Right-stage return and harness shield point |
| 9 | `R_PWMA` | `GP8` | Right TB6612FNG `PWMA` | Front-right speed command |
| 10 | `R_AIN1` | `GP9` | Right TB6612FNG `AIN1` | Front-right direction A |
| 11 | `R_AIN2` | `GP10` | Right TB6612FNG `AIN2` | Front-right direction B |
| 12 | `R_BIN1` | `GP11` | Right TB6612FNG `BIN1` | Rear-right direction A |
| 13 | `R_BIN2` | `GP12` | Right TB6612FNG `BIN2` | Rear-right direction B |
| 14 | `R_PWMB` | `GP13` | Right TB6612FNG `PWMB` | Rear-right speed command |

### PDB bill of material

| Qty | Item | Suggested value or spec | Notes |
| --- | --- | --- | --- |
| 1 | Battery input connector | JST-VH 2-pin, vertical or right-angle to suit layout | Mates with the battery-holder harness |
| 1 | Main fuse holder plus fuse | PCB fuse holder plus 7.5 A blade fuse | Protects the full robot supply |
| 1 | Master power switch | Latching SPST, at least 10 A DC | Full system power disconnect |
| 1 | Reverse-polarity protection stage | Ideal-diode MOSFET stage or equivalent controller plus MOSFET | Prevents battery reversal damage |
| 1 | Battery TVS diode | Unidirectional TVS for a 2S Li-ion rail | Clamps hot-plug and wiring transients |
| 1 | 5.1 V buck converter | 5 A minimum, low-ripple synchronous buck module or proven regulator stage | Source for both `AUX_5V` and the protected `PI_5V` branch |
| 1 | Raspberry Pi output protection stage | Resettable fuse or eFuse/current-limited load switch sized for the Pi 3B supply | In series with `PI_5V` |
| 1 | 6.0 V servo buck converter | 5 A peak minimum | Keeps servo surge current off the logic rail |
| 1 | 3.3 V driver logic regulator | Small LDO or regulator, about 150 mA or higher | Local-only `DRV_3V3` for both TB6612FNG `VCC` pins and the `STBY` pull-up |
| 2 | TB6612FNG motor-driver stages | One left and one right | Mount on the PDB beside the motor connectors |
| 1 | `STBY` pull-up resistor | 10 kOhm, 0603, to local `3V3` | Enables both TB6612FNG stages by default |
| 1 | `STBY` test pad or solder jumper | Normally left enabled | Useful during bring-up and fault isolation |
| 1 | Battery-input bulk capacitor | 470 uF low-ESR, 16 V or higher | Close to the protected battery entry |
| 1 | 5.1 V source bulk capacitor | 470 uF low-ESR, 10 V or higher | Close to the 5.1 V buck output before the branch split |
| 1 | Raspberry Pi connector bulk capacitor | 100 uF or higher, low-ESR, 10 V or higher | Close to `J_PI_PWR` after the protection stage |
| 1 | 6.0 V rail bulk capacitor | 470 uF low-ESR, 10 V or higher | Close to the 6.0 V buck output |
| 2 | Driver VM bulk capacitors | 220 uF low-ESR, 16 V or higher, one per TB6612FNG stage | Local motor surge reservoir on `MOTOR_VM` |
| 2 | Driver VM ceramic capacitors | 100 nF, 0603, one per driver | Place directly at each TB6612FNG `VM` pin |
| 2 | Driver VCC ceramic capacitors | 100 nF, 0603, one per driver | Place directly at each TB6612FNG `VCC` pin |
| 4 | Miscellaneous ceramic capacitors | 100 nF, 0603 | Battery entry, buck/regulator bypassing, and connector-side local bypassing |
| 6 | Rail indicator LEDs | 0603 | `VBAT`, `VBAT_PROT`, `MOTOR_VM`, `AUX_5V`, `PI_5V`, and `SERVO_6V` |
| 6 | LED resistors | 2.2 kOhm, 0603 | One per indicator LED |
| 1 | Control-board auxiliary connector | JST-VH 4-pin | `AUX_5V`, `SERVO_6V`, `GND`, `GND` |
| 1 | Control-board driver-control connector | JST-XH 14-pin | Single low-current control harness |
| 1 | Raspberry Pi power connector | JST-VH 2-pin | Dedicated Pi feed |
| 4 | Motor connectors | JST-VH 2-pin | One connector per wheel motor |
| 11 | Test points | `VBAT`, `VBAT_PROT`, `MOTOR_VM`, `AUX_5V`, `PI_5V`, `SERVO_6V`, `DRV_3V3`, `STBY`, and three grounds | Bring-up and troubleshooting |

## Control board

### High-level overview

- Mount the Raspberry Pi Pico 2 directly on the board and keep its USB connector accessible for programming and Raspberry Pi USB data.
- Bring `AUX_5V`, `SERVO_6V`, and two grounds in through `J_PWR_AUX`.
- Split `AUX_5V` into two local branches: `AUX_5V` -> reverse-blocking ideal-diode stage -> `PICO_VSYS`, and `AUX_5V` -> input bulk capacitor -> ferrite bead -> `5V_FILT`.
- Place `47 uF + 100 nF` directly after the ferrite bead and feed the LCD and both HC-SR04 connectors from `5V_FILT`.
- Feed the Pico 2 only at `VSYS` from `PICO_VSYS` and use the Pico-generated `3V3` rail as the only control-board 3.3 V rail.
- Keep the Pi 3B link as the Pico's native USB device connection. Leave USB `VBUS` on the USB connector for attach detect and programming, but do not short USB `VBUS` to `AUX_5V`, `5V_FILT`, or `PICO_VSYS`.
- Keep the HC-SR04 level shifting on the control board beside the Pico and the sensor connectors, and place one 100 nF ceramic directly at each HC-SR04 connector.
- Send all wheel-motor control signals to the PDB through the single 14-pin JST-XH harness using the fixed `GP2` through `GP13` mapping shown above.
- Use JST-VH for servo outputs and JST-XH for the HC-SR04, LCD, and PDB driver-control harnesses.
- Keep the control harness away from the ultrasonic echo lines where practical.

### Control-board connector plan

| Ref | Connector | Family | Pinout | Notes |
| --- | --- | --- | --- | --- |
| `J_PWR_AUX` | PDB auxiliary input | JST-VH 4-pin | `AUX_5V`, `SERVO_6V`, `GND`, `GND` | Logic and servo supply feed |
| `J_DRV_CTRL` | PDB driver control | JST-XH 14-pin | See motor-control harness pin map above | Single low-current control harness to the PDB |
| `J_SERVO_PAN` | Pan servo | JST-VH 3-pin | `PWM`, `SERVO_6V`, `GND` | Robust 3-wire servo harness |
| `J_SERVO_TILT` | Tilt servo | JST-VH 3-pin | `PWM`, `SERVO_6V`, `GND` | Robust 3-wire servo harness |
| `J_US1` | Forward-left HC-SR04 | JST-XH 4-pin | `5V`, `TRIG`, `ECHO`, `GND` | Sensor harness |
| `J_US2` | Forward-right HC-SR04 | JST-XH 4-pin | `5V`, `TRIG`, `ECHO`, `GND` | Sensor harness |
| `J_LCD` | LCD1602 harness | JST-XH 12-pin | `VSS`, `VDD`, `VO`, `RS`, `RW`, `E`, `D4`, `D5`, `D6`, `D7`, `LEDA`, `LEDK` | Leaves `D0` to `D3` unconnected |

### Control-board bill of material

| Qty | Item | Suggested value or spec | Notes |
| --- | --- | --- | --- |
| 1 | Raspberry Pi Pico 2 footprint | Direct-solder Pico 2, no socket | Keep the USB connector mechanically accessible |
| 1 | Input bulk capacitor before ferrite | 100 uF, 10 V or higher | Place at `J_PWR_AUX` on the incoming `AUX_5V` rail |
| 1 | Pico `VSYS` ideal-diode stage | Reverse-blocking ideal diode or PFET-based ideal-diode circuit sized for Pico current | Feeds `PICO_VSYS` from `AUX_5V` without backfeeding the board from USB `VBUS` |
| 1 | Ferrite bead on `AUX_5V` input | 0603 ferrite bead sized for the LCD and HC-SR04 current | Forms the `5V_FILT` branch |
| 1 | Filtered 5 V bulk capacitor | 47 uF, 10 V or higher | Place directly after the ferrite bead |
| 3 | Filtered 5 V ceramic capacitors | 100 nF, 0603 | One after the ferrite bead, one at `J_US1`, and one at `J_US2` |
| 4 | Local 100 nF ceramics | 0603, near Pico and level-shifter sections | Local logic decoupling |
| 1 | Pico bulk capacitor | 10 uF, 10 V or higher | Near the Pico power pins |
| 4 | BSS138 MOSFETs | One per HC-SR04 TRIG or ECHO path | Integrated 4-channel level shifter |
| 8 | Level-shifter pull-up resistors | 10 kOhm, 0603 | Four to 3.3 V and four to 5 V |
| 2 | HC-SR04 connectors | JST-XH 4-pin | `5V`, `TRIG`, `ECHO`, `GND` |
| 1 | LCD connector | JST-XH 12-pin | Main LCD harness |
| 1 | LCD contrast potentiometer | 10 kOhm trimmer | Generates `VO` locally |
| 1 | LCD backlight resistor footprint | 220 Ohm, 0603, populate only if the LCD module lacks its own resistor | Series limit for `LEDA` |
| 2 | Servo connectors | JST-VH 3-pin | Pan and tilt outputs |
| 1 | PDB auxiliary input connector | JST-VH 4-pin | `AUX_5V`, `SERVO_6V`, `GND`, `GND` |
| 1 | PDB motor-control connector | JST-XH 14-pin | Single low-current control harness |
| 1 | Filtered 5 V status LED plus resistor | 0603 LED plus 2.2 kOhm, 0603 | Confirms `5V_FILT` is present on the board |
| 1 | 3.3 V status LED plus resistor | 0603 LED plus 2.2 kOhm, 0603 | Confirms the Pico logic rail is alive |
| 8 | Test points | `AUX_5V`, `PICO_VSYS`, `USB_VBUS`, `5V_FILT`, `SERVO_6V`, `3V3`, `GND`, and one PWM line per side | Debug and scope access |

## Harness BOM

### Harness standards

- Standardize all board-side high-current connectors on JST-VH with vertical top-entry through-hole headers: `B2P-VH`, `B3P-VH`, and `B4P-VH`.
- Standardize all board-side signal connectors on JST-XH with right-angle side-entry through-hole headers: `S4B-XH-A`, `S12B-XH-A`, and `S14B-XH-A`.
- Standardize JST-VH harness contacts on `SVH-41T-P1.1`, which suits the chosen `18 AWG` and `20 AWG` stranded wire plan.
- Standardize JST-XH harness contacts on `SXH-001T-P0.6`, using `26 AWG` stranded wire for the HC-SR04 and LCD harnesses.
- Use `18 AWG` stranded wire for battery input, raw motor feed, and the four wheel-motor leads.
- Use `20 AWG` stranded wire for Raspberry Pi power, board-to-board auxiliary power, and both servo harnesses.
- Use `26 AWG` stranded wire for the HC-SR04, LCD, and PDB motor-control harnesses.

### Orientation and pin-1 rules

- All JST-VH board headers are vertical top-entry parts. Place them on the board edge with the mating face accessible from above the PCB.
- All JST-XH board headers are right-angle side-entry parts. Place them on the board edge so the cable exits parallel to the PCB and toward the enclosure wall, including the two 14-pin driver-control connectors.
- Mark pin 1 on every PCB footprint with a silkscreen triangle and keep the schematic pin order identical to the physical cavity order.
- For every power connector, pin 1 is the positive rail. For duplicated grounds, grounds follow after the positive rails.
- For every harness drawing, view pin order from the mating face of the cable housing, not from the wire-entry side.

### Board-side connector BOM

| Qty | Exact part | Series | Orientation | Used at |
| --- | --- | --- | --- | --- |
| 6 | `B2P-VH` | JST-VH | Vertical, top-entry, THT | `J_BAT`, `J_PI_PWR`, `J_MOTOR_FL`, `J_MOTOR_RL`, `J_MOTOR_FR`, `J_MOTOR_RR` |
| 2 | `B3P-VH` | JST-VH | Vertical, top-entry, THT | `J_SERVO_PAN`, `J_SERVO_TILT` |
| 2 | `B4P-VH` | JST-VH | Vertical, top-entry, THT | `J_CTRL_AUX`, `J_PWR_AUX` |
| 2 | `S4B-XH-A` | JST-XH | Right-angle, side-entry, THT | `J_US1`, `J_US2` |
| 1 | `S12B-XH-A` | JST-XH | Right-angle, side-entry, THT | `J_LCD` |
| 2 | `S14B-XH-A` | JST-XH | Right-angle, side-entry, THT | `J_CTRL_DRV`, `J_DRV_CTRL` |

### Harness-side connector BOM

This counts the board-mating housings and crimp contacts needed for one full robot build. Module-end terminations for the Raspberry Pi, LCD glass, HC-SR04 boards, and motor tabs stay separate from this count.

| Qty | Exact part | Series | Use |
| --- | --- | --- | --- |
| 6 | `VHR-2N` | JST-VH | Battery input, Raspberry Pi power, and four motor-output harnesses |
| 2 | `VHR-3N` | JST-VH | Two servo harnesses |
| 2 | `VHR-4N` | JST-VH | Board-to-board auxiliary power harness |
| 2 | `XHP-4` | JST-XH | HC-SR04 board-end housings |
| 1 | `XHP-12` | JST-XH | LCD board-end housing |
| 2 | `XHP-14` | JST-XH | Board-to-board driver-control harness |
| 26 | `SVH-41T-P1.1` | JST-VH | All VH crimp contacts for the board-mating cable ends |
| 48 | `SXH-001T-P0.6` | JST-XH | All XH crimp contacts for the board-mating cable ends |

### Off-board assembly parts

| Qty | Item | Suggested value or spec | Notes |
| --- | --- | --- | --- |
| 4 | Motor terminal suppression capacitors | 100 nF ceramic, X7R, 50 V or higher | Solder directly across each wheel-motor terminal pair with the shortest practical leads |

### Per-harness build list

| Harness | Qty | Board-side mating parts | Wire | Pin order |
| --- | --- | --- | --- | --- |
| Battery holder -> `J_BAT` | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `18 AWG` red and black | `1=VBAT`, `2=GND` |
| PDB `J_CTRL_AUX` -> control `J_PWR_AUX` | 1 cable | `2 x VHR-4N`, `8 x SVH-41T-P1.1` | `20 AWG` for all four conductors | `1=AUX_5V`, `2=SERVO_6V`, `3=GND`, `4=GND` |
| PDB `J_CTRL_DRV` -> control `J_DRV_CTRL` | 1 cable | `2 x XHP-14`, `28 x SXH-001T-P0.6` | `26 AWG` 14-conductor ribbon or bundled hookup wire | `1=GND`, `2=GP2/L_PWMA`, `3=GP3/L_AIN1`, `4=GP4/L_AIN2`, `5=GP5/L_BIN1`, `6=GP6/L_BIN2`, `7=GP7/L_PWMB`, `8=GND`, `9=GP8/R_PWMA`, `10=GP9/R_AIN1`, `11=GP10/R_AIN2`, `12=GP11/R_BIN1`, `13=GP12/R_BIN2`, `14=GP13/R_PWMB` |
| PDB `J_PI_PWR` -> Raspberry Pi power pigtail or adapter | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `20 AWG` red and black | `1=PI_5V`, `2=GND` |
| PDB `J_MOTOR_FL` -> front-left motor | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `18 AWG` pair | `1=A01`, `2=A02` |
| PDB `J_MOTOR_RL` -> rear-left motor | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `18 AWG` pair | `1=B01`, `2=B02` |
| PDB `J_MOTOR_FR` -> front-right motor | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `18 AWG` pair | `1=A01`, `2=A02` |
| PDB `J_MOTOR_RR` -> rear-right motor | 1 cable | `1 x VHR-2N`, `2 x SVH-41T-P1.1` | `18 AWG` pair | `1=B01`, `2=B02` |
| Control `J_SERVO_PAN` -> pan servo adapter lead | 1 cable | `1 x VHR-3N`, `3 x SVH-41T-P1.1` | `20 AWG` three-conductor | `1=PWM`, `2=SERVO_6V`, `3=GND` |
| Control `J_SERVO_TILT` -> tilt servo adapter lead | 1 cable | `1 x VHR-3N`, `3 x SVH-41T-P1.1` | `20 AWG` three-conductor | `1=PWM`, `2=SERVO_6V`, `3=GND` |
| Control `J_US1` -> forward-left HC-SR04 harness | 1 cable | `1 x XHP-4`, `4 x SXH-001T-P0.6` | `26 AWG` four-conductor | `1=5V`, `2=TRIG`, `3=ECHO`, `4=GND` |
| Control `J_US2` -> forward-right HC-SR04 harness | 1 cable | `1 x XHP-4`, `4 x SXH-001T-P0.6` | `26 AWG` four-conductor | `1=5V`, `2=TRIG`, `3=ECHO`, `4=GND` |
| Control `J_LCD` -> LCD harness | 1 cable | `1 x XHP-12`, `12 x SXH-001T-P0.6` | `26 AWG` 12-conductor ribbon or bundled hookup wire | `1=VSS`, `2=VDD`, `3=VO`, `4=RS`, `5=RW`, `6=E`, `7=D4`, `8=D5`, `9=D6`, `10=D7`, `11=LEDA`, `12=LEDK` |

### Practical notes

- Using JST-VH on the servo outputs means these become custom servo adapter leads, not direct mates to common hobby-servo plugs.
- Route the 14-pin JST-XH driver-control harness as a logic interconnect. Keep it separated from the four motor-output harnesses where practical instead of bundling it tightly alongside them over long runs.
- The HC-SR04 and LCD module ends are not standardized to JST footprints in this build. Terminate them with a dedicated adapter PCB or fixed pigtail as required by the module.
- Use `SVH-41T-P1.1` only with the specified `18 AWG` and `20 AWG` VH harnesses. Do not substitute `22 AWG` without changing the contact family.

## Notes

- No transformer is needed on either board. These are DC-to-DC buck stages, so the magnetic part is an inductor inside the regulator design or module.
- `0603` is the default only for the small passives. Bulk capacitors, trimmers, buck stages, fuse hardware, and the TB6612FNG thermal copper need larger packages and footprints.
- The PDB `DRV_3V3` rail and the control-board `3V3` rail are separate local rails. Connect the boards only through the defined grounds and the 14 control signals.
- The Pi 3B to motion-controller link stays on USB. No extra UART or USB-serial bridge is required for normal communication or programming; add SWD only if you want low-level debug access.
- Because the motor rail is raw 2S, verify wheel-motor stall current at full charge, not nominal battery voltage. The TB6612FNG is efficient enough that this matters.

