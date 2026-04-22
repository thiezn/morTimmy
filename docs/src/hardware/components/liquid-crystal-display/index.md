# Liquid Crystal Display

This came with the Arduino Starter Kit.

Model is LCM1602C V2.1

## Overview

This is a standard 16x2 character LCD module in the HD44780 family. I could not find a trustworthy public datasheet for the exact `LCM1602C V2.1` board revision, so the details below are based on common HD44780-compatible 1602 modules and line up with how the module is wired in the project schematic.

## Key characteristics

- 16 characters x 2 rows
- 5x8 dot matrix per character
- HD44780-compatible command set
- Supports 8-bit and 4-bit parallel modes
- Typical logic supply: 5 V
- Typical logic current: around 1 mA to 2 mA without backlight
- Backlight uses separate LED pins

## Pinout

| Pin | Name | Function |
| --- | --- | --- |
| 1 | VSS | Ground |
| 2 | VDD | +5 V logic supply |
| 3 | VO / VE | Contrast input, usually from a potentiometer wiper |
| 4 | RS | Register select, command or data |
| 5 | RW | Read or write, often tied to ground for write-only use |
| 6 | E | Enable strobe |
| 7 | D0 | Data bit 0 |
| 8 | D1 | Data bit 1 |
| 9 | D2 | Data bit 2 |
| 10 | D3 | Data bit 3 |
| 11 | D4 | Data bit 4 |
| 12 | D5 | Data bit 5 |
| 13 | D6 | Data bit 6 |
| 14 | D7 | Data bit 7 |
| 15 | LEDA | Backlight LED anode |
| 16 | LEDK | Backlight LED cathode |

## How it is used in this project

The display is wired in 4-bit write-only mode:

- `RS` and `E` go to Pico GPIO
- `D4` to `D7` carry the 4-bit data bus
- `RW` is tied to ground
- `D0` to `D3` are left unconnected
- `VO` is driven by a 10 kOhm contrast potentiometer
- `LEDA` and `LEDK` power the backlight

## Electrical notes

- Common operating range for these modules is about 4.7 V to 5.3 V
- Input high level is compatible with common 5 V LCD logic
- Contrast is normally adjusted with a potentiometer between 5 V and ground, with the wiper on `VO`
- Some modules include a backlight resistor on the PCB and some do not; if yours does not, add a series resistor on `LEDA`

## Mechanical notes

Typical 1602 modules are about 80 mm x 36 mm with a 16-pin 0.1 inch header footprint.

## References used

- Generic HD44780-compatible 16x2 LCD module references
- Components101 16x2 LCD pinout summary
- Futurlec 16x2 LCD technical data for a compatible module

