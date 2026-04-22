# State Machines

Treat embedded peripherals and robot behaviors as explicit state machines.

## Peripherals

- List the valid states.
- List the valid transitions.
- Make illegal transitions impossible or explicit errors.

For hardware-facing APIs, this often means a mix of typestate at configuration time and narrow runtime methods once configured.

## Robot Behavior

For control software, use runtime enums and transition methods.

- `Idle`, `Teleop`, `Autonomous`, and `Fault` are runtime states.
- Operator input, timeouts, reconnects, and telemetry can all trigger transitions.
- Keep transitions centralized so side effects happen in one place.

## Mortimmy Rule

Mode changes and autonomy progression belong in the host or firmware runtime state machine, not in the protocol codec layer.