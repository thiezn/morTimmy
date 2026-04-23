# Driver Traits

Driver traits should expose capabilities, not board wiring details.

## Trait Shape

- Keep methods small and intention-revealing.
- Use shared units and domain types.
- Avoid catch-all methods with many optional parameters.
- Prefer idempotent setters or narrow commands over broad mutable bags of state.

## Design Contracts

Apply compile-time contracts where possible.

- Make capabilities explicit in the trait surface.
- Split traits when read and write responsibilities differ.
- Use newtypes and validated constructors for physical units and bounded values.

Apply runtime contracts where needed.

- Reject impossible hardware requests early.
- Re-clamp on the embedded side.
- Surface failures with domain errors instead of silent fallback.

This follows the embedded Rust design-contract guidance: use the type system to prevent misuse when practical, and keep runtime checks for the parts that genuinely depend on live hardware state.
