# Contributing to pico-conduit

Thank you for your interest in contributing! This document covers everything you need to get
started: prerequisites, build commands, code style, and the branching workflow.

## Prerequisites

> **Tip:** The fastest way to get started is the included [Dev Container](.devcontainer/) —
> open this repo in VS Code or GitHub Codespaces and all tools are pre-installed.

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
cargo test --test host --no-default-features --target aarch64-unknown-linux-musl

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
- **Test placement:** `tests/host/` for Tier 1 & 2 public API tests (run with
  `cargo test --test host`); `tests/integration/` for Tier 4 TCP tests (require hardware,
  all `#[ignore]`).

## Branching Workflow

1. Fork the repository and create a branch: `git checkout -b feat/short-description`
2. Keep commits small and focused; write imperative-mood commit messages.
3. Ensure `cargo test --test host --no-default-features --target aarch64-unknown-linux-musl` and `cargo clippy` pass before opening a PR.
4. Open a pull request against `master` and fill in the PR template.

## Adding a New Peripheral Interface

1. Create `src/interfaces/<name>.rs` following the pattern of an existing interface.
2. Add a `pub mod <name>;` line in `src/interfaces/mod.rs`.
3. Add the interface name and its valid actions to the match table in `src/router.rs`.
4. Add tests in `tests/host/interfaces/<name>.rs` using `embedded-hal-mock`.
5. Add `mod <name>;` to `tests/host/interfaces/mod.rs`.
6. Update `PROTOCOL.md` with the interface spec.

## Release Process

1. Move `[Unreleased]` entries to a new versioned section in `CHANGELOG.md`.
2. Bump `version` in `Cargo.toml` (root package).
3. Run Tier 1 & 2 tests locally: `cargo test --test host --no-default-features --target aarch64-unknown-linux-musl`
4. Run Tier 3 & 4 tests manually on hardware.
5. Push the `vx.y.z` tag to trigger the CI `release` job, which produces `pico-conduit.uf2`.
6. Copy the CHANGELOG section into the GitHub Release description.

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

## Code of Conduct

This project follows the [Contributor Covenant v2.1](CODE_OF_CONDUCT.md). By participating
you agree to abide by its terms.
