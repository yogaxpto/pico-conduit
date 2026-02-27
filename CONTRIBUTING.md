# Contributing to pico-socketeer

Thank you for your interest in contributing! This document covers everything you need to get
started: prerequisites, build commands, code style, and the branching workflow.

## Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust stable | ≥ 1.85 | `rustup toolchain install stable` |
| thumbv8m target | latest | `rustup target add thumbv8m.main-none-eabihf` |
| flip-link | latest | `cargo install flip-link` — stack-overflow-safe linker |
| probe-rs | latest | `cargo install probe-rs-tools` — flash + RTT |
| CYW43 firmware blobs | — | see [README.md § Build](README.md#build) |

## Build Commands

```sh
# Build firmware
cargo build --release --target thumbv8m.main-none-eabihf

# Run firmware on connected hardware via probe-rs
cargo run --release --target thumbv8m.main-none-eabihf

# Host unit + mock hardware tests (Tier 1 + 2, no hardware needed)
cargo test

# Format check
cargo fmt --check

# Lint (treat warnings as errors)
cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings
```

## Code Style

- **No heap allocation:** no `Box`, `Vec`, `String`, or `alloc` — use `heapless::*`.
- **Embassy async only:** use `embassy-executor`, `embassy-time`, `embassy-sync`. Do not
  use `cortex-m::asm::wfe` or raw spinloops.
- **Structured logging:** use `defmt::*` macros (`info!`, `warn!`, `error!`). Do not use
  `println!` or `log::*`.
- **Error codes:** interface-level errors are `&'static str` literals from the catalogue in
  `src/protocol.rs`. Do not add new error strings not in the catalogue without updating
  `PROTOCOL.md`.
- **`no_std` purity:** `src/lib.rs` modules must compile on both the embedded target and
  the host test runner. Embedded-only code lives in `src/net.rs` (guarded by
  `[target.'cfg(target_os = "none")'.dependencies]` in `Cargo.toml`).
- **Test placement:** in-file `#[cfg(test)]` modules for private logic; `tests/host/` for
  public API integration tests.

## Branching Workflow

1. Fork the repository and create a branch: `git checkout -b feat/short-description`
2. Keep commits small and focused; write imperative-mood commit messages.
3. Ensure `cargo test` and `cargo clippy` pass before opening a PR.
4. Open a pull request against `master` and fill in the PR template.

## Adding a New Peripheral Interface

1. Create `src/interfaces/<name>.rs` following the pattern of an existing interface.
2. Add a `pub mod <name>;` line in `src/interfaces/mod.rs`.
3. Add the interface name and its valid actions to the match table in `src/router.rs`.
4. Add `#[cfg(test)]` tests using `embedded-hal-mock` in the new file.
5. Update `PROTOCOL.md` with the interface spec.

## Commit Message Format

```
<type>(<scope>): <short summary>

<body — wrap at 72 chars>
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `ci`, `chore`.

Example:
```
feat(gpio): add pull-down mode to set_mode action
```
