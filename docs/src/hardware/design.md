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

All voltage regulation, protection, and branching occurs on this board.  
No downstream module is allowed to create or cascade its own primary supply unless explicitly required.

### Power Topology

    Battery Pack (single source)
    │
    ├── High‑current rail (raw or bucked)
    │   └── Motor Drivers (L298N or future replacements)
    │
    ├── Medium‑current rail (6V)
    │   └── Servo connectors
    │
    └── Logic rails
        ├── 5V → Raspberry Pi, USB devices, audio modules
        └── 3.3V → Microcontrollers, sensors

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

*   Battery connector (XT30 / XT60)
*   Main fuse
*   Reverse polarity protection (MOSFET or diode)
*   Buck converters:
    *   Battery → 6V (servos)
    *   Battery → 5V (logic)
    *   5V → 3.3V (logic, if not buck‑derived)
*   Bulk capacitors near each rail
*   Measurement points for each rail
*   Ground plane with star‑ground topology


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
                        │ • 6V Buck (Servos)  │
                        │ • 5V Buck (Logic)   │
                        │ • 3.3V Regulator    │
                        │ • Central Ground    │
                        └───┬────────┬─────┬─┘
                            │        │     │
                   ┌────────▼───┐    │     │
                   │ Motor Ctrl  │    │     │
                   │ Pico        │    │     │
                   └────────────┘    │     │
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
               [ Reverse Polarity Protection ]
                        │
                ┌────────┼─────────┐
                │        │         │
            [Buck]   [Buck]    [Buck/LDO]
            Batt→Motor Batt→6V   5V→3V3
                │        │         │
             Motor     Servo      Logic
             Rail      Rail       Rail
                │        │         │
          Motor Drivers  Servos    MCUs + Sensors

### Grounding Strategy

*   One **main ground bus** on the PDB
*   All rails return to this point
*   No daisy‑chained grounds
*   Sensitive logic grounds remain physically distant from motor current paths


## 6. Data and Interconnect Strategy

### Power and Data Separation

Power delivery and data signaling are always routed separately except where standards require otherwise (e.g. servo headers).

### Data Buses

| Bus          | Use Case               | Pros                         | Cons                                |
| ------------ | ---------------------- | ---------------------------- | ----------------------------------- |
| USB          | Pi ↔ Pico              | High bandwidth, power + data | Connector bulk, grounded noise path |
| UART         | Pi ↔ Pico              | Simple, deterministic        | Point‑to‑point                      |
| I²C          | Sensors                | Minimal wiring               | Noise sensitive, short distance     |
| SPI          | High‑speed peripherals | Fast, robust                 | More wires                          |
| CAN (future) | Distributed modules    | Robust, fault tolerant       | Extra transceivers                  |


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
| **XT30 / XT60**              | High‑current battery connectors                              |
| **JST Connectors**           | Compact connectors for low‑power logic                       |
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
