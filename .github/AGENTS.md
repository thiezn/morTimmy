# Mortimmy

- Rust workspace with a Raspberry Pi host daemon, RP2350 firmware, and shared `core`, `drivers`, and `protocol` crates.
- Continuous control uses one desired-state snapshot; one-shot protocol commands cover ping, status, params and audio.
- Start with `docs/src/architecture/architecture.md`, `docs/src/architecture/protocol.md`, and `TODO.md`.
- Validate with `cargo test --workspace --color never`; embedded-only check: `cargo check -p mortimmy-rp2350 --target thumbv8m.main-none-eabihf`.
- Build docs with `./scripts/book.sh`
