---
name: embedded-rust-programs
description: "Hardware driver and firmware development best practices for embedded Rust"
---

# Embedded Rust Programs

Use this skill when designing or changing firmware control loops, board drivers, protocol-owned control state, or embedded-facing host abstractions.

## Core Pattern

Prefer one latest-wins desired-state owner for continuous control.

- The host or higher-level controller owns the desired state.
- The firmware owns the applied state and is the final safety authority.
- One-shot actions such as ping, logging, audio chunks, or parameter updates stay outside the continuous-control path.

For mortimmy, the default pattern is:

1. Build one explicit desired-control snapshot.
2. Clamp and validate it once.
3. Send it as an idempotent message.
4. Apply it through one firmware method.
5. Emit one acknowledgement telemetry type for that apply path.

## Typestate Guidance

Use typestate selectively.

- Use typestate for initialization, ownership transfer, and configuration sequences that should be impossible to misuse at compile time.
- Use runtime state machines for robot modes, autonomy plans, reconnect logic, and other behavior that must react to live inputs or telemetry.

If a state can legitimately change because of runtime events, it usually belongs in a runtime state machine, not in a typestate API.

## Driver And Firmware Rules

- Keep driver traits narrow and capability-oriented.
- Make invalid states hard to represent.
- Keep one apply path per control domain.
- Distinguish `stationary` from `idle` explicitly.
- Enforce limits in firmware even when the host clamps first.
- Prefer full desired-state snapshots over patch messages until payload size proves otherwise.
- Keep telemetry shaped around acknowledgement and observability, not around transport accidents.

## Review Checklist

- Is continuous control modeled as latest-wins state rather than a queue of imperative commands?
- Is there exactly one owner of desired state and one owner of applied state?
- Are safety limits enforced on the embedded side?
- Are runtime robot modes represented with explicit transitions?
- Is typestate only being used where compile-time guarantees actually help?
- Do tests cover roundtrip, latest-wins semantics, and timeout behavior?

## References

- See `reference/desired-control.md` for the mortimmy control pattern.
- See `reference/typestate.md` for when typestate helps and when it hurts.
- See `reference/state-machines.md` for runtime state-machine guidance.
- See `reference/driver-traits.md` for trait shape and design-contract rules.
