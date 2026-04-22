# Pico Audio Pack

Pimoroni Pico Audio Pack is the audio add-on mounted on the Pico 2 W.

## Purpose

This board turns the Pico 2 W into the robot's audio controller. It accepts I2S digital audio from the Pico, converts it to analog audio, and exposes both line-level and headphone outputs.

## Main parts

- PCM5100A stereo DAC
- PAM8908JER stereo headphone amplifier
- AP7333-33SRG 3.3 V regulator
- 3.5 mm stereo line-out jack
- 3.5 mm stereo headphone jack
- Gain switch for the headphone amplifier

## Pin usage on the Pico

The Pico Audio Pack consumes the Pico's I2S pins.

| Pico GPIO | Audio Pack signal | Notes |
| --- | --- | --- |
| GP9 | I2S data / DIN | Serial audio data into the PCM5100A |
| GP10 | I2S bit clock / BCK / SCK | I2S clock |
| GP11 | I2S word select / LRCK / WS | Left-right channel clock |
| GP29 | MUTE / amp enable | Used by Pimoroni examples to silence or enable output |

The schematic and Pimoroni software examples both line up with this mapping. In example code, the pack is typically configured as `data=GP9`, `bclk=GP10`, and `lrck=GP11`.

## Wiring notes

- The board plugs directly onto the Pico header set.
- Audio is carried over I2S from the Pico to the DAC.
- The LCD in this project is placed on other spare Pico GPIO so GP9-GP11 remain reserved for audio.
- The board exposes analog output only; it is not a speaker power stage for large passive speakers.

## Capabilities

- Stereo DAC output up to 32-bit / 384 kHz according to the PCM5100A device used on the board
- Headphone output through the onboard amplifier
- Line-level output for an external amplifier or powered speakers

## Mechanical notes

- Approximate dimensions: 53 mm x 29 mm x 11 mm
- Designed as a Pico add-on pack with pre-soldered female headers

## Sources used

- Pimoroni product page
- Pimoroni Pico Audio Pack schematic PDF
- Pimoroni example code showing `GP9`, `GP10`, and `GP11` I2S usage
