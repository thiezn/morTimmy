# More ideas without priority

- consider robot vacuum wheel base instead of four wheels. Ask for other stable options and pros and cons
- camera image with Gemma 4 and that segmenting flow that can draw things o. Top of image would be cool
- teleoperated through iPhone with video would be fucking great. Especially if we allow multiple input controllers so we can use gamepad to the robot with video on phone.
- design firmware update architecture. The robot should expose one usbc port that we can connect to. The pi will accept update commands and will update the firmware of all microcontrollers. It should also be able to update itself. It should also be able to poll an update server to fetch updates so that logic needs to be separated.


## Documentation and schematics

- tsserial for custom PCB design (for instance, I'd like to have some kind of power board)

## Homekit support

UPDATE on the below!

Apple is now supporting matter protocol so we don't need custom homekit. This makes it easier to also integrate into other homekit like ecosystems. There's also a active rust crate using embassy for matter support:

https://github.com/sysgrok/rs-matter-embassy

We will adopt this instead of rebuilding hap-rs for no-std support. This also means we can have homekit/matter support on the host side without needing to maintain a custom hap-rs fork.

OLD:

the rust hap-rs crate is not really maintained anymore but might allow us to integate with homekit.

Maybe we can fork it, and transform it so we can use it with embassy as that does support async so the port would perhaps be doable? That would unlock rust in microcontrollers with no-std!!!

https://github.com/ewilken/hap-rs/issues/40

Either way, it would also be nice to have homekit support on the host side if thats the only feasible way at the moment. Its also perhaps less practical for my robot, although adding a flashlite to it would be cool, as well as having buttons on the robot that appears as switches is also cool.

