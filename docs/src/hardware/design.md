# Hardware System Architecture and Power Design

## 1. System Architecture Overview

This robot is designed as a **layered hardware system**. Each layer has a clearly defined responsibility, explicit interfaces, and controlled dependencies. Power is treated as a first‑class subsystem rather than an afterthought.

### Layered Hardware Model

    Mechanical Structure
    └── Power System
        ├── High‑power actuation rails (motors)
        ├── Medium‑power actuation rails (servos)
        ├── Logic power rails (MCUs, SBCs, sensors)
    └── Compute & Control
        ├── Main processor (SBC / Raspberry Pi)
        ├── Real‑time controllers (microcontrollers)
    └── Interconnect
        ├── Power distribution
        ├── Data buses and signaling
    └── Peripherals
        ├── Motors
        ├── Servos
        ├── Sensors
        ├── Audio and UI components

Each layer may evolve independently as long as **power interfaces and signal contracts remain stable**.


## 2. Power Architecture

### Design Decision

The system uses **a single battery source with centralized power distribution**, implemented through a dedicated **Power Distribution Board (PDB)**

All primary system rails originate on this board.  
The only local conversion off-board is the Pico 2 onboard 3.3 V regulator, used only for control-board logic.

The control board feeds the Pico 2 at `VSYS` from `AUX_5V` through a reverse-blocking ideal-diode path. The Pi 3B still reaches the Pico over the Pico USB connector for programming and runtime communication.

### Power Topology

    Battery Pack (2S single source)
    │
    ├── Protected raw rail (`MOTOR_VM`)
    │   └── Two TB6612FNG motor-driver stages on the PDB
    │
    ├── Medium‑current rail (`SERVO_6V`)
    │   └── Servo connectors
    │
    └── Logic rails
        ├── `PI_5V` → Raspberry Pi
        ├── `AUX_5V` → Control board `VSYS` feed and 5 V loads
        ├── `DRV_3V3` → PDB-local TB6612 logic
        └── Pico `3V3` → Control-board MCU and level shifting

### Rail Separation Rationale

*   Motors generate electrical noise and large current spikes
*   Servos generate medium transient loads
*   Logic components require clean and stable voltage

Separating rails **prevents brown‑outs, resets, and signal corruption** while maintaining a shared reference ground.


## 3. Power Distribution Board (PDB)

### Role

The Power Distribution Board is the **electrical anchor** of the system.

It has no compute logic and performs no signaling functions beyond power delivery and monitoring.

### Core Responsibilities

*   Accept battery input
*   Provide reverse‑polarity and over‑current protection
*   Generate required voltage rails
*   Distribute power through dedicated connectors
*   Provide a central ground reference

### Typical Components on the PDB

*   Battery connector (JST-VH in the current board plan)
*   Main fuse
*   Master power switch
*   Reverse polarity protection (MOSFET or diode)
*   5.1 V buck converter for `PI_5V` and `AUX_5V`
*   6.0 V buck converter for `SERVO_6V`
*   Local 3.3 V regulator for TB6612FNG `VCC` and `STBY`
*   Two TB6612FNG stages on the PDB beside the wheel-motor connectors
*   Bulk capacitors near the battery entry, the 5.1 V rail, the 6.0 V rail, and each TB6612FNG stage
*   Measurement points for each rail
*   Ground plane with controlled high-current return paths and stitching vias around the driver region


## 4. System Architecture

                         ┌────────────────────┐
                         │    Battery Pack    │
                         └─────────┬──────────┘
                                   │
                        ┌──────────▼──────────┐
                        │ Power Distribution  │
                        │ Board (PDB)         │
                        │─────────────────────│
                        │ • Fuse              │
                        │ • Polarity protect  │
                        │ • TB6612FNG x2      │
                        │ • 6V Buck (Servos)  │
                        │ • 5.1V Buck         │
                        │ • DRV_3V3 Regulator │
                        │ • Central Ground    │
                        └───┬────────┬─────┬─┘
                            │        │     │
                    ┌───────▼──────┐ │     │
                    │ Control Board│ │     │
                    │ Pico + LCD + │ │     │
                    │ US + USB dev │ │     │
                    └──────────────┘ │     │
                                     │     │
                             ┌───────▼┐ ┌──▼─────────┐
                             │ Servos │ │ Raspberry  │
                             │ (6V)   │ │ Pi (5V)    │
                             └────────┘ └────────────┘

All components connect **inwards** to the power system rather than chaining power between devices.


## 5. Detailed Power Rail Schematic (Conceptual)

                    Battery (+)
                        │
                    [ Main Fuse ]
                        │
                   [ Master Switch ]
                        │
               [ Reverse Polarity Protection ]
                        │
                ┌────────┼──────────────┬──────────────┐
                │        │              │              │
          `MOTOR_VM`  [Buck 5.1V]   [Buck 6.0V]   [Reg 3.3V]
                │        │              │              │
         TB6612FNG x2  `PI_5V` +     Servo Rail    `DRV_3V3`
                    │       `AUX_5V`         │              │
            Wheel Motors  │            Servos      TB6612 VCC/STBY
                          │
                  Raspberry Pi + Control Board
                          │
                    `AUX_5V` -> ideal diode -> Pico `VSYS`
                    USB `D+`/`D-`/`VBUS` -> Pico USB connector

### Grounding Strategy

*   One **main ground bus** on the PDB
*   All rails return to this point
*   No daisy‑chained grounds
*   Sensitive logic grounds remain physically distant from motor current paths


## 6. Data and Interconnect Strategy

### Power and Data Separation

Power delivery and data signaling are always routed separately except where standards require otherwise (e.g. servo headers).

For the motion controller, keep USB as the primary Pi 3B to Pico 2 interface. The control board powers the Pico from `AUX_5V` into `VSYS`, while the Pi USB host connection still carries `D+`, `D-`, `GND`, shield, and host `VBUS` to the Pico USB connector for attach detect and programming.

### Data Buses

| Bus          | Use Case               | Pros                         | Cons                                |
| ------------ | ---------------------- | ---------------------------- | ----------------------------------- |
| USB          | Pi ↔ Pico              | High bandwidth, power + data | Connector bulk, grounded noise path |
| UART         | Pi ↔ Pico              | Simple, deterministic        | Point‑to‑point                      |
| I²C          | Sensors                | Minimal wiring               | Noise sensitive, short distance     |
| SPI          | High‑speed peripherals | Fast, robust                 | More wires                          |
| CAN (future) | Distributed modules    | Robust, fault tolerant       | Extra transceivers                  |

### Forward obstacle sensing

The motion controller now owns a dual ultrasonic sensing pair built from two HC-SR04 modules.

*   One sensor is mounted forward-left at roughly 45 degrees.
*   One sensor is mounted forward-right at roughly 45 degrees.
*   Both sensors are powered from the 5V logic rail.
*   Both sensors share one Adafruit 4-channel BSS138 level converter.

The current pin and signal contract is:

*   GP14 / GP15: forward-left `TRIG` and `ECHO`
*   GP16 / GP17: forward-right `TRIG` and `ECHO`
*   The level converter uses all four BSS138 channels: left `TRIG`, left `ECHO`, right `TRIG`, right `ECHO`

The firmware polls the two HC-SR04 modules sequentially rather than simultaneously. That avoids overlapping trigger bursts and reduces acoustic crosstalk between the left and right forward cones.


## 7. Design Rules

These rules define the system’s long‑term robustness.

1.  Power generation occurs **only on the Power Distribution Board**
2.  Motors never share a rail with logic components
3.  Servos have their own dedicated voltage rail
4.  No cascaded regulators unless explicitly designed
5.  One shared ground with controlled return paths
6.  Every rail is measurable and fuseable
7.  Connectors are preferred over soldered wires
8.  Breadboards are prohibited for final power delivery
9.  Mechanical mounting and wiring paths are designed together
10. All connectors are keyed or polarized


## 8. Terminology Reference

| Term                         | Description                                                  |
| ---------------------------- | ------------------------------------------------------------ |
| **Power Rail**               | A regulated voltage supply distributed throughout the system |
| **Buck Converter**           | A switching regulator that efficiently steps voltage down    |
| **Regulator (LDO)**          | Linear voltage regulator, low noise but inefficient          |
| **Star Grounding**           | Ground topology where all returns meet at one physical point |
| **Main Ground Bus**          | Central low‑impedance ground reference on the PDB            |
| **Dupont Wires**             | Temporary jumper wires used only during prototyping          |
| **Servo**                    | Actuator with control + power combined in a 3‑wire interface |
| **Motor Driver**             | High‑current interface between logic signals and motors      |
| **JST-VH / JST-XH**          | Board and harness connectors used for power and signal wiring |
| **Power Distribution Board** | Dedicated PCB handling all system power                      |
| **Data Bus**                 | Shared signaling interface between devices                   |


## 9. Resulting System Properties

By enforcing this architecture, the system achieves:

*   Electrically stable operation
*   Modular expansion capability
*   Mechanical and electrical coherence
*   Predictable behavior under load
*   Clear separation of responsibilities
*   A maintainable and professional hardware platform

This design forms the foundation for future iterations, higher integration, and long‑term reliability.
