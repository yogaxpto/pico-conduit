# pico-socketeer — Project Objective & Implementation Plan

## Goal

Build a Rust firmware for the **Raspberry Pi Pico 2W** (RP235x) that connects to a Wi-Fi network and exposes an asynchronous message-passing interface. Incoming messages are dispatched to the appropriate hardware interface (GPIO, UART, SPI, I2C, PWM, ADC, USB); outgoing messages report state or results back to the network peer.

The device acts as a **network-controlled hardware bridge**: a remote client sends JSON (or binary) commands over a socket, and the Pico executes them against real peripherals and replies asynchronously.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│                  Raspberry Pi Pico 2                 │
│                                                      │
│  ┌────────────┐     ┌──────────────────────────────┐ │
│  │  CYW43 Wi-Fi│────▶│  TCP Socket / Message Queue  │ │
│  │  (SPI bus) │     └──────────────┬───────────────┘ │
│  └────────────┘                    │ dispatch         │
│                          ┌─────────▼──────────┐      │
│                          │   Message Router   │      │
│                          └──┬──┬──┬──┬──┬──┬─┘      │
│                GPIO ────────┘  │  │  │  │  │        │
│                UART ───────────┘  │  │  │  │        │
│                SPI ───────────────┘  │  │  │        │
│                I2C ──────────────────┘  │  │        │
│                PWM ─────────────────────┘  │        │
│                ADC/USB ────────────────────┘        │
└──────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1 — Toolchain & HAL Foundation

**Goal:** Get a working, buildable project that boots on the Pico 2.

- [ ] Pin `rp235x-hal` and required BSP crate in `Cargo.toml`
- [ ] Add `embassy-rp` (async executor) as the async runtime
- [ ] Configure the linker script (`memory.x`) for the RP235x's flash/RAM layout
- [ ] Set correct build target (`thumbv8m.main-none-eabihf`) and `.cargo/config.toml` flags
- [ ] Verify a `defmt`-based logging setup for debug output over USB serial
- [ ] Boot to a stable idle loop; confirm via `probe-rs` or UF2 flash

**Key crates:**
- `rp235x-hal` — peripheral drivers for the RP235x
- `embassy-rp` — async runtime and executor for RP235x
- `embassy-executor` — task scheduling
- `defmt` + `defmt-rtt` — structured logging
- `panic-probe` — panic handler that logs via probe

---

### Phase 2 — Wi-Fi Connectivity (CYW43439)

**Goal:** Join a Wi-Fi network and open a TCP socket.

- [ ] Integrate `cyw43` driver (CYW43439 chip on Pico W / Pico 2 W)
- [ ] Load the CYW43 firmware blob at link time
- [ ] Implement `embassy-net` network stack (IP + TCP)
- [ ] Store SSID / password in a `config.toml` or compile-time env vars (`env!`)
- [ ] Perform DHCP lease on connect; expose IP over serial log
- [ ] Open a listening TCP socket on a configurable port (default `4242`)
- [ ] Implement a keep-alive / reconnect loop for dropped connections

**Key crates:**
- `cyw43` — CYW43439 Wi-Fi driver
- `embassy-net` — `no_std` TCP/IP stack (built on `smoltcp`)
- `smoltcp` — underlying network stack
- `heapless` — fixed-capacity data structures for `no_std`

---

### Phase 3 — Message Protocol

**Goal:** Define a simple, extensible wire format for commands and responses.

#### Message Format (JSON over TCP, newline-delimited)

**Command (client → Pico):**
```json
{
  "id": "abc123",
  "interface": "gpio",
  "action": "write",
  "pin": 15,
  "value": 1
}
```

**Response (Pico → client):**
```json
{
  "id": "abc123",
  "ok": true,
  "data": null
}
```

#### Interfaces & Actions

| Interface | Actions |
|-----------|---------|
| `gpio` | `read`, `write`, `set_mode` (input/output/pull) |
| `uart` | `write`, `read`, `configure` (baud, parity, stop bits) |
| `spi` | `transfer`, `write`, `configure` |
| `i2c` | `write`, `read`, `write_read`, `configure` |
| `pwm` | `set_duty`, `set_freq`, `enable`, `disable` |
| `adc` | `read` (returns raw 12-bit value or voltage) |
| `usb` | `write` (CDC serial), `read` |

- [ ] Define `Command` and `Response` structs
- [ ] Implement a `no_std`-compatible JSON parser (using `serde-json-core`)
- [ ] Implement framing: newline-delimited records over the TCP stream
- [ ] Validate commands and return structured errors on bad input

**Key crates:**
- `serde` (with `derive`, `no_std`) — serialization framework
- `serde-json-core` — `no_std` JSON with no heap allocation

---

### Phase 4 — Peripheral Drivers & Message Router

**Goal:** Wire each interface to the message router.

- [ ] **GPIO**: configure pin direction/pull, digital read/write
- [ ] **UART**: configure and write/read bytes over UART0 or UART1
- [ ] **SPI**: full-duplex transfers over SPI0 or SPI1
- [ ] **I2C**: master read/write/write_read over I2C0 or I2C1
- [ ] **PWM**: set frequency and duty cycle per slice/channel
- [ ] **ADC**: read channels 0–3 (GPIO26–29) and the onboard temperature sensor
- [ ] **USB CDC**: read/write to the virtual serial port over USB

Each driver module exposes an `async fn handle(cmd: &Command) -> Response` function.
The router dispatches based on `cmd.interface` and awaits the result.

---

### Phase 5 — Async Task Model

**Goal:** Run networking and peripheral handling concurrently without blocking.

- [ ] Spawn an `embassy` task for the Wi-Fi driver (`cyw43` background task)
- [ ] Spawn a task for the `embassy-net` network stack
- [ ] Spawn a task for the TCP listener (accepts connections, reads framed messages)
- [ ] Spawn a task per active connection (or a bounded pool) for message handling
- [ ] Use `embassy::channel::Channel` for inter-task communication (command queue, response queue)
- [ ] Ensure peripheral access is protected with `embassy::mutex::Mutex` where needed

---

### Phase 6 — Configuration & Provisioning

**Goal:** Allow runtime or compile-time configuration without reflashing.

- [ ] Compile-time config via `env!` macros (SSID, password, port) read from a `.env` file at build time
- [ ] Optional: store config in the last page of flash using `sequential-storage` or `embedded-storage`
- [ ] Expose a `GET /config` endpoint (or a special command) to report current settings
- [ ] Optional: a provisioning mode (AP mode) if no config is found in flash

---

### Phase 7 — Testing & Validation

Testing is structured in four tiers, each runnable independently. A clean `cargo build` or `cargo build --release` **never** includes test code or mock peripherals — all test isolation is enforced by Rust's standard `#[cfg(test)]` and `[dev-dependencies]` mechanisms.

#### Tier 1 — Host Unit Tests (no hardware required)

Compile and run protocol and routing logic on the host machine using the standard Rust test harness.

```sh
cargo test --target x86_64-unknown-linux-gnu
```

- Tests message parsing, JSON framing, command validation, and routing dispatch
- All test modules are gated with `#[cfg(test)]` and excluded from firmware builds
- No `no_std` constraint in test context — host tests use `std`

- [ ] Write `#[test]` cases for `Command` / `Response` serialization and deserialization
- [ ] Write `#[test]` cases for framing edge cases (partial reads, oversized messages, malformed JSON)
- [ ] Write `#[test]` cases for router dispatch (correct handler called for each `interface` value)

#### Tier 2 — Mock Hardware Tests (no hardware required)

Inject fake peripheral implementations using `embedded-hal-mock` to test each interface driver's logic in isolation on the host.

```sh
cargo test --target x86_64-unknown-linux-gnu
```

- `embedded-hal-mock` is a `[dev-dependency]` — it is **never** linked into the firmware binary
- Each interface module (GPIO, UART, SPI, I2C, PWM, ADC) is written against `embedded-hal` traits, making them testable with mock backends
- Mock peripheral expectations are set up per test and asserted after the command executes

- [ ] Add `embedded-hal-mock` to `[dev-dependencies]` in `Cargo.toml`
- [ ] Write mock-backed tests for each interface driver: verify the correct HAL calls are made for each action
- [ ] Test error paths: invalid pin numbers, out-of-range values, busy peripherals

#### Tier 3 — On-Device Tests via USB / probe-rs (real hardware required)

Run tests compiled for the RP235x and flashed via `probe-rs`. Output is captured over RTT using `defmt`.

```sh
cargo test --target thumbv8m.main-none-eabihf
```

Requires `.cargo/config.toml` to set the test runner for the embedded target:

```toml
[target.thumbv8m.main-none-eabihf]
runner = "probe-rs run --chip RP235x"
```

And `defmt-test` as a `[dev-dependency]`:

```toml
[dev-dependencies]
defmt-test = "0.3"
```

- [ ] Verify GPIO read/write on real pins
- [ ] Verify ADC reads return plausible values
- [ ] Verify UART loopback (TX → RX with a jumper wire)
- [ ] Test Wi-Fi join and DHCP lease
- [ ] Test TCP listener accepts a connection and echoes a message

#### Tier 4 — TCP Integration Tests (real hardware + network required)

Flash the firmware normally, then run a Rust test binary on the host that connects to the Pico's TCP socket and exercises the full message protocol end-to-end.

```sh
# Flash firmware first
cargo build --release
probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/pico-socketeer

# Then run integration tests (host binary, uses std::net::TcpStream)
PICO_IP=192.168.1.x cargo test --test integration
```

Located in `tests/integration/tcp_client.rs`. This binary is a standard Rust integration test; it compiles for the host and is **never** included in the firmware image.

- [ ] Test GPIO write → read round-trip via TCP
- [ ] Test ADC read returns a valid response
- [ ] Test malformed JSON returns a structured error response
- [ ] Test reconnect: drop and re-open TCP connection, verify the device recovers
- [ ] Measure round-trip latency: command sent → response received
- [ ] Test Wi-Fi loss simulation (disconnect AP) and automatic reconnect

---

#### Test Binary Isolation (Important)

`cargo build` and `cargo build --release` produce firmware that contains **zero test or mock code**:

| Mechanism | Effect |
|-----------|--------|
| `#[cfg(test)]` | Test modules compiled only when `cargo test` is invoked |
| `[dev-dependencies]` | `embedded-hal-mock`, `defmt-test` never linked into firmware |
| `tests/` directory | Cargo compiles these as separate host binaries, not as firmware |
| Default build target | `.cargo/config.toml` sets `thumbv8m.main-none-eabihf`; host tests require `--target x86_64-unknown-linux-gnu` |

---

### Phase 8 — Packaging & Distribution

- [ ] Build script that produces a `.uf2` file flashable via USB drag-and-drop
- [ ] Document the wire protocol and all commands in a `PROTOCOL.md`
- [ ] Provide a reference client library in Rust (usable from any host connecting via TCP)
- [ ] CI pipeline (GitHub Actions) that:
  - Builds the firmware (`cargo build --release --target thumbv8m.main-none-eabihf`)
  - Runs Tier 1 & 2 host tests (`cargo test --target x86_64-unknown-linux-gnu`)
  - Produces a `.uf2` artifact on each tagged release

---

## Directory Structure (target)

```
pico-socketeer/
├── src/
│   ├── main.rs          # Entry point, executor, task spawning
│   ├── net.rs           # Wi-Fi init, TCP listener
│   ├── protocol.rs      # Command / Response types, JSON framing
│   ├── router.rs        # Dispatch commands to interface handlers
│   └── interfaces/
│       ├── gpio.rs
│       ├── uart.rs
│       ├── spi.rs
│       ├── i2c.rs
│       ├── pwm.rs
│       ├── adc.rs
│       └── usb.rs
├── tests/
│   ├── host/
│   │   └── protocol_tests.rs   # Tier 1 & 2: host unit + mock tests (cargo test --target x86_64)
│   └── integration/
│       └── tcp_client.rs       # Tier 4: TCP integration test (PICO_IP=... cargo test --test integration)
├── memory.x             # Linker script for RP235x
├── build.rs             # Build script (links memory.x, embeds CYW43 firmware)
├── Cargo.toml
├── OBJECTIVE.md         # This file
└── PROTOCOL.md          # Wire protocol reference (Phase 8)
```

---

## Non-Goals (v1)

- TLS/encrypted transport (planned for v2)
- Over-the-air firmware updates
- Multi-device mesh networking
- A web UI or REST API (TCP sockets only in v1)

---

## Key Constraints

| Constraint | Detail |
|------------|--------|
| No heap | `no_std` + `no alloc`; all buffers are statically sized with `heapless` |
| Single core | RP235x is dual Cortex-M33; v1 uses core 0 only via `embassy` |
| Flash size | 4 MB on Pico 2; CYW43 firmware blob is ~230 KB |
| RAM | 520 KB SRAM; message buffers must be conservatively sized |
| Async only | All I/O is non-blocking via `embassy`; no RTOS threads |
