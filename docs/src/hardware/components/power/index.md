# Power System Design

The current electrical design uses a dedicated power distribution board fed from a 2S battery pack.

## Final board targets

- Battery input on a 2-pin JST-VH connector
- Protected raw battery rail `MOTOR_VM` for the two TB6612FNG stages
- 5.1 V buck output split into protected `PI_5V` for the Raspberry Pi and `AUX_5V` for the control board
- Dedicated `SERVO_6V` buck rail for the pan and tilt servos
- Local `DRV_3V3` rail on the PDB for TB6612FNG `VCC` and `STBY`
- Battery-entry bulk capacitance, per-driver `220 uF + 100 nF` motor decoupling, and dedicated Raspberry Pi output protection

## Prototype parts on hand

- DIY 2-Slot 18650 Battery Holder with Pins – Black
- AC Charger + 2xUltraFire 18650 3.7V 3000mAh Rechargeable Battery
- 3A-6S UBEC 'hobbywing' for powering the Raspberry Pi and stabilizing the voltage
- UBEC or equivalent buck module for the 6 V servo rail
