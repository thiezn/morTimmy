# More ideas without priority

## Documentation and schematics

- tsserial for custom PCB design (for instance, I'd like to have some kind of power board)

## Homekit support

the rust hap-rs crate is not really maintained anymore but might allow us to integate with homekit.

Maybe we can fork it, and transform it so we can use it with embassy as that does support async so the port would perhaps be doable? That would unlock rust in microcontrollers with no-std!!!

https://github.com/ewilken/hap-rs/issues/40

Either way, it would also be nice to have homekit support on the host side if thats the only feasible way at the moment. Its also perhaps less practical for my robot, although adding a flashlite to it would be cool, as well as having buttons on the robot that appears as switches is also cool.
