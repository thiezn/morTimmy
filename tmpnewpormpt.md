# dingen die ik zie tijdens deployment

- messages in protocol can waarschijnlijk ook verder opgesplits worden in ui, actuators, sensors + een aantal meer categorieen
- De main loop voor mortimmy moet ControlLoop heten, dit is de term die we in firmware gebruike en klinkt duidelijk
- Ik ben niet helemaal 100% blij met de naam app in mortimmy, wat is dit precies? hoe valt dit samen met control loop?
- Voor de firmware van rp2350, link_rx/tx hebben nu dingen als trellis en audio hardcoded. Maar ik heb zometeen twee pico's met dezelfde firmware. Alle pheripherals zouden feature flags moeten zijn OF we moeten aparte firmware maken voor beide raspberry pi's (ze zijn ook niet identiek in versie dus is dat nodig?)
- De led blinking op de firmware is erg useful om te begrijpen wat er misgaat. Laten we vragen om een robuuste implementatie die na de eerste vier lange blinks, meerdere blinks geeft die de status weergeeft. Deze blink code moet blijven repeaten met een langere tussenpose zodat we het kunnen blijven zien als we de code gemist hadden.

## Dingen die ik moet gaan leren

type state pattern is volgens mij erg veel gebruikt om de robot/sensor/motor state te coderen. Hoe werkt embasssy hiermee? heeft het al abstracties? Daarnaast, repr'c op structs om ervoor te zorgen dat memory layout hetzelfde blijft.



# Eerste Prompt 

- teleoperated and autonomous mode
- pico should only receive desiredstate. One big message that tells it what the desired state is. If it has multiple messages, it should use the latest desired state. This should make the logic on the pico simpler and more robust.
- the brain can switch between teleoperated and autonomous mode. Autonomous mode would allow you to tell the brain to execute a certain set of desired states with conditions (time exceeded, sensor reading ==,>=!= etc). Initially we will hardcode some of these patterns but we also eventually want to be able to take these in through the websocket interface so we can remotely add new automations without rebuilding the software.

I suspect the Rust TypeState pattern can really help in the design to ensure the robot cannot end up in states that are invalid. 

More information about good design practices can be found here:
- https://docs.rust-embedded.org/book/static-guarantees/index.html
- https://docs.rust-embedded.org/book/static-guarantees/typestate-programming.html
- https://docs.rust-embedded.org/book/static-guarantees/state-machines.html
- https://docs.rust-embedded.org/book/static-guarantees/design-contracts.html


Review all relevant code and create a strong design plan for this, ask me questions if there are tradeoffs i need to be aware of and implement it on the pico and mortimmy. Update all relevant unit and integration tests.

Finally, update the architecture/protocol docs with this information and create sequence mermaid diagrams to explain the flows between the pi and pico when using the new driver. Create a consise skill that explains the common pattern and traits we should use when developing a drivers or firmware in @SKILL.md




## volgende prompt:

I am almost ready to hook up the motor controllers and power infrastructure to the raspberry pi pico. Focus on the @file:drive.rs firmware implementation and make it a robust implementation that should actually work. Try to test as such as possible (unit tests) the implementation without having the actual controller connected.

The controller we will use is the @file:l298n-stepper-motor-driver . Make sure we have a full implementation of the driver in the drivers crate. Create sub folders for specific driver implementations to keep code organized.

Think of a good way how we can integrate specific driver implementations into the firmware. We need to be able to easily swap out drivers in the future without changing the abstractions. Propose a solution for this. I am considering at least putting sensor, actuator and ui capabilities in the firmware all behind feature flags so we can compile different versions of the firmware according to our needs. Think about how this would tie into the specific driver implementations as well.

When implementing a new driver, think about leveraging the rust type state pattern to ensure that the driver cannot be used in an invalid state. For example, if the driver needs to be initialized before it can be used, we can use the type state pattern to enforce this at compile time.

More information about good design practices for embedded rust can be found here:
- https://docs.rust-embedded.org/book/static-guarantees/index.html
- https://docs.rust-embedded.org/book/static-guarantees/typestate-programming.html
- https://docs.rust-embedded.org/book/static-guarantees/state-machines.html
- https://docs.rust-embedded.org/book/static-guarantees/design-contracts.html

Finally, update the architecture/protocol docs with this information and create sequence mermaid diagrams to explain the flows between the pi and pico when using the new driver. Create a consise skill that explains the common pattern and traits we should use when developing a drivers or firmware in @SKILL.md
