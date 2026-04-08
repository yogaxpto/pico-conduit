# pico-conduit — AI Coding Conventions

This file documents conventions that AI coding assistants (Claude, Copilot, etc.) **must**
follow when contributing to this project.

## Hard Rules

### No heap allocation
Never use `Box`, `Vec`, `String`, `format!`, `alloc::*`, or any `std` type that allocates.
Use `heapless::Vec<u8, N>`, `heapless::String<N>`, or fixed-size arrays for all dynamic
data.

### Embassy async only
Do not use `cortex_m::asm::wfe`, `cortex_m::asm::delay`, raw spinloops, or thread::sleep.
All blocking waits must be `embassy_time::Timer::after_*` or similar embassy primitives.

### `defmt` logging only
Do not use `println!`, `eprintln!`, `log::info!`, or the `std` `log` crate.
All log output must use `defmt::info!`, `defmt::warn!`, `defmt::error!`, `defmt::debug!`,
or `defmt::trace!`.

### `no_std` purity for library code
`src/lib.rs` and all modules it declares must compile on both the embedded target
(`thumbv8m.main-none-eabihf`) and the host test runner (`aarch64-unknown-linux-musl`).
Embedded-only code belongs in `src/net.rs` or behind
`[target.'cfg(target_os = "none")'.dependencies]` in `Cargo.toml`.

### Error codes are `&'static str`
Interface-level errors are `&'static str` constants from `src/protocol.rs`.
Do not define new error string literals outside that catalogue.
Always update `PROTOCOL.md` when adding new error codes.

### Single connection constraint
The TCP/WebSocket server accepts exactly one client at a time. The MQTT transport
processes one command at a time (single-command-at-a-time over the broker).
Do not add concurrency primitives that would allow simultaneous connections.

### Transport feature flags are mutually exclusive
Exactly one of `transport-tcp`, `transport-websocket`, or `transport-mqtt` must be
enabled for embedded builds. Enabling two simultaneously triggers a `compile_error!`.
The default feature set enables `transport-tcp`. Transport-specific code in `src/net.rs`
is gated with `#[cfg(feature = "transport-*")]`.

## Code Conventions

- **Lifetimes:** when a function returns `Response<'_>`, use an explicit lifetime `'a`
  tied to the data source (not the reference), e.g.
  `fn foo<'a>(cmd: &Command<'a>) -> Response<'a>`.
  This is required by Rust 2024 edition's stricter lifetime elision rules.

- **Serde:** use `serde-json-core` (not `serde_json`). It has no support for
  `deserialize_any`; use concrete type hints (`deserialize_u64`, etc.) in custom
  `Deserialize` implementations.

- **Peripheral validation:** each interface handler validates its own required fields
  (pin, channel, address, etc.) and returns `Response::error(cmd.id, ERROR_*)` on
  failure. The router only validates `interface` and `action`.

- **GPIO29 reserved:** always reject pin 29 with `ERROR_INVALID_PIN` — it is the CYW43
  SPI DIO line and must not be touched by the JSON protocol.

- **ADC channel encoding:** `adc_channel` is an integer: `0` = GPIO26, `1` = GPIO27,
  `2` = GPIO28, `3` = onboard temperature sensor.

## Testing

- All new logic must have `#[cfg(test)]` tests.
- Use `embedded-hal-mock` with the `eh1` feature for Tier 2 peripheral tests.
- Run `cargo test` (host triple, no `--target`) before every commit.
- The test command is **not** `cargo test --target thumbv8m.main-none-eabihf`.

## Linker and Memory

- `memory.x` defines four regions: `BOOT2`, `FLASH`, `CREDENTIALS` (8 KB for
  `sequential-storage`), and `RAM`.
- The picotool binary-info sections (`.boot_info`, `.bi_entries`) must remain in
  `memory.x`; do not remove them.
- Stack overflow protection is provided by `flip-link`; never switch to a different linker
  without updating `.cargo/config.toml` and verifying the overflow protection still works.

## Build Targets

- **Pico 2W (default):** `thumbv8m.main-none-eabihf` — feature `pico2w`
- **Pico W:** `thumbv6m-none-eabi` — feature `pico1w`
- **Host tests:** `aarch64-unknown-linux-musl` (or native host triple) — no `embedded` feature

## Wiki

The GitHub wiki lives in a **separate Git repository** cloned into the `wiki/` folder
of this project:

```
git clone https://github.com/yogaxpto/pico-conduit.wiki.git wiki
```

The `wiki/` folder is **not** part of the main `pico-conduit` repo — it has its own
`.git` directory and its own commit history. Do not commit `wiki/` contents to the main
repo.

### Batch agent workflow

When wiki pages are written by batch / parallel agents:

1. Each agent works on its assigned page(s) and creates a commit in the `wiki/` repo.
2. Before pushing, each agent must **rebase onto the main branch** of the wiki repo:
   ```bash
   cd wiki && git fetch origin && git rebase origin/master
   ```
3. Resolve any conflicts (unlikely for separate pages, but handle if they arise).
4. Push the rebased commits.

### File naming

Wiki pages are Markdown files in the `wiki/` root. GitHub wiki derives the page title
from the filename, so use `kebab-case` with `.md` extension:

- `Home.md` (landing page — GitHub requires this exact name)
- `Hardware-Setup.md`
- `Flashing-the-Firmware.md`
- `Wi-Fi-Provisioning.md`
