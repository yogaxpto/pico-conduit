# Changelog

All notable changes to pico-conduit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Embassy async runtime (embassy-rp 0.9.0, embassy-net 0.8.0, cyw43 0.6.0) replacing
  the original cortex-m-rt + panic-halt scaffold
- Newline-delimited JSON wire protocol v1 (`src/protocol.rs`) with full framing,
  serialisation/deserialisation, and 13-entry error catalogue
- Message router (`src/router.rs`) validating `interface` + `action` fields
- Peripheral interface stubs for GPIO, UART, SPI, I2C, PWM, ADC, and USB
  (`src/interfaces/`)
- Flash credential storage stub (`src/provisioning/storage.rs`)
- Provisioning portal stub (`src/provisioning/portal.rs`)
- LED state machine with 8 states including SOS Morse error pattern (`src/led.rs`)
- Wi-Fi STA mode with DHCP, exponential reconnect backoff, and power management
  (`src/net.rs`)
- TCP server on port 4242 with 30-second idle timeout
- Tier 1 unit tests (155 tests) covering protocol, router, led, storage, and portal
- Tier 2 mock hardware tests for all interface handlers using `embedded-hal-mock`
- CI pipeline: `lint` → `build-and-test` → `release` jobs
- `PROTOCOL.md`, `LED_STATUS.md`, `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`
- RP2350 memory layout with 8 KB credentials flash region (`memory.x`)
- picotool binary-info linker sections for UF2 metadata
