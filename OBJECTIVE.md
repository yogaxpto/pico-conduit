# pico-socketeer — Project Objective & Implementation Plan

## Goal

Build a Rust firmware for the **Raspberry Pi Pico 2W** (RP235x) that connects to a Wi-Fi network and exposes an asynchronous message-passing interface. Incoming messages are dispatched to the appropriate hardware interface (GPIO, UART, SPI, I2C, PWM, ADC, USB); outgoing messages report state or results back to the network peer.

The device acts as a **network-controlled hardware bridge**: a remote client sends JSON commands over a socket, and the Pico executes them against real peripherals and replies asynchronously.

---

## Getting Started

> All tools are pre-installed in the devcontainer. No host-side Rust or embedded toolchain setup is required.

**1. Open the devcontainer**
- VS Code: `Dev Containers: Reopen in Container` (Command Palette)
- CLI: `devcontainer up --workspace-folder .`

**2. Set development credentials (optional but recommended)**
Copy `.env.example` to `.env` and fill in your local Wi-Fi SSID and password. These are injected at compile time (see Phase 6a) and skip flash storage during development:
```sh
cp .env.example .env
# edit .env with your local AP credentials
source .env
```

**3. Essential commands**
| Task | Command |
|------|---------|
| Build firmware | `cargo build --release` |
| Flash via probe-rs (SWD) | `probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/pico-socketeer` |
| Flash via UF2 (bootloader) | `elf2uf2-rs target/thumbv8m.main-none-eabihf/release/pico-socketeer` |
| Host unit + mock tests | `cargo test --target x86_64-unknown-linux-gnu` |
| Lint | `cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings` |
| Format check | `cargo fmt --check` |

Full contributor guide (branching, release process, code style) will be in `CONTRIBUTING.md` — see Phase 8b. Until it exists, the commands above cover day-to-day development.

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│                  Raspberry Pi Pico 2W                │
│                                                      │
│  ┌─────────────┐    ┌──────────────────────────────┐ │
│  │ CYW43 Wi-Fi │───▶│  TCP Socket / Message Queue  │ │
│  │  (SPI bus)  │    └──────────────┬───────────────┘ │
│  │             ├── LED (GPIO0) ◀── │ led_task        │
│  └─────────────┘   status light    │ dispatch        │
│                          ┌─────────▼──────────┐      │
│                          │   Message Router   │      │
│                          └──┬──┬──┬──┬──┬──┬──┘      │
│                GPIO ────────┘  │  │  │  │  │         │
│                UART ───────────┘  │  │  │  │         │
│                SPI ───────────────┘  │  │  │         │
│                I2C ──────────────────┘  │  │         │
│                PWM ─────────────────────┘  │         │
│                ADC/USB ────────────────────┘         │
└──────────────────────────────────────────────────────┘
```

### Startup / Provisioning Flow

```
Boot                              [LED: 3-flash burst]
 │
 ├─ BOOTSEL held ≥ 5 s? ──▶ YES ──▶ erase credentials flash sector
 │                                   [LED: 5 rapid flashes → OFF]
 │                                        │
 │                                   resume boot (no credentials path)
 │
 ├─ load_credentials() from flash
 │
 ├─ [credentials found] ──▶ CYW43 STA mode ──▶ DHCP ──▶ TCP socket server (port 4242)
 │        [LED: fast blink 5 Hz]                              [LED: solid ON]
 │
 └─ [no credentials]    ──▶ CYW43 AP mode
          [LED: slow blink 1 Hz]   │  SSID: "pico-setup-XXXX" (last 4 hex of MAC)
                                   │  IP:   192.168.4.1
                                   │
                              HTTP captive portal (port 80)
                                   │
                GET /  ──▶ scan SSIDs, render HTML form
                      [LED: double-blink while scanning]
                POST /connect ──▶ attempt STA join
                      [LED: fast blink 5 Hz]
                                   │
                    ┌──────────────┴─────────────┐
                    │ success                    │ failure
                    ▼                            ▼
        serve confirmation page         serve error page
        (SSID + IP + countdown)         (reason + retry link)
                    │                            │
        flush response to client        return to AP mode
                    │                   [LED: slow blink 1 Hz]
        save_credentials() to flash
          │ success                │ failure
          ▼                        ▼
  [LED: 5 rapid flashes → OFF]   serve storage-error page
          │                       return to AP mode
  watchdog reboot → STA mode     [LED: slow blink 1 Hz]

  (if reconnection fails repeatedly)  [LED: SOS · · · — — — · · ·]
```

---

## Phase Dependencies

The table below shows which phases must be completed (or stubbed) before a given phase can begin. Forward references are marked with `(stub)` — a minimal placeholder implementation is sufficient to unblock the dependent phase.

| Phase | Depends on |
|-------|------------|
| 1 — Toolchain | — |
| 2 — Wi-Fi | 1, 6a (stub) |
| 3 — Protocol | 1 |
| 4 — Peripheral Drivers | 1, 3 |
| 5a — Async Task Model | 2, 3, 4 |
| 5b — LED | 5a, 2 |
| 5c — Power Management | 2, 5a |
| 6a — Flash Storage | 1 |
| 6b–6e — Provisioning | 2, 6a |
| 6f — Factory Reset | 6a, 5b |
| 7 — Testing | All implementation phases |
| 8 — Packaging | 7 |

**Suggested implementation order:**

> Follow this sequence to unblock each phase as early as possible. Stubs (marked below) are minimal placeholders sufficient to satisfy the dependency without full functionality.

1. **Phase 1** — Toolchain & HAL foundation (including `.vscode/settings.json`, devcontainer update, minimal CI)
2. **Phase 6a (stub)** — Flash storage stub returning `None` from `load_credentials()` — unblocks Phase 2
3. **Phase 2** — Wi-Fi connectivity
4. **Phase 3** — Message protocol
5. **Phase 4** — Peripheral drivers & router
6. **Phase 5a** — Async task model
7. **Phase 5b** — LED signaling
8. **Phase 5c** — Power management
9. **Phase 6a (full)** — Complete flash storage implementation
10. **Phases 6b–6f** — Provisioning, captive portal, factory reset
11. **Phase 7** — Testing & validation
12. **Phase 8** — Packaging, documentation, CI expansion

---

## Implementation Phases

### Phase 1 — Toolchain & HAL Foundation

**Goal:** Get a working, buildable project that boots on the Pico 2.

> **Start here:** Create `rust-toolchain.toml` before any other task. Without it, `cargo build` uses whatever Rust version the devcontainer happens to have, making the toolchain non-reproducible across environments and CI. All subsequent checklist items assume this file is in place.

> **Scaffold replacement note:** The current scaffold (`src/main.rs`, `Cargo.toml`) uses `cortex-m-rt`'s `#[entry]` macro and `panic-halt`. These are **incompatible** with Embassy and must be replaced wholesale — not extended. Specifically: remove `cortex-m`, `cortex-m-rt`, and `panic-halt` from `[dependencies]`; replace `#[entry]` with `#[embassy_executor::main]`; add `panic-probe`. Embassy manages `cortex-m`/`cortex-m-rt` as transitive dependencies — do not re-add them as direct dependencies.

- [ ] Add `rust-toolchain.toml` at repo root pinning Rust stable channel, components (`rustfmt`, `clippy`, `rust-src`), and target (`thumbv8m.main-none-eabihf`); this file is read by rustup, the devcontainer, and CI to ensure all environments use an identical toolchain
- [ ] Add `embassy-rp` (async runtime + HAL) and its required dependencies in `Cargo.toml`; **do not add `rp235x-hal`** — `embassy-rp` IS the HAL for embassy on RP2350 and the two HALs conflict; also remove the scaffold's `cortex-m`, `cortex-m-rt`, and `panic-halt` entries and replace with `panic-probe`
- [ ] Configure the linker script (`memory.x`) for the RP235x's flash/RAM layout
- [ ] Add the following to `.cargo/config.toml` under a `[target.thumbv8m.main-none-eabihf]` section:
  - `rustflags = ["-C", "link-arg=-Tlink.x", "-C", "link-arg=--nmagic"]` — required for `memory.x` to be found by the linker; without `-Tlink.x` the binary will fail to link
  - `linker = "flip-link"` — stack overflow detection via the pre-installed `flip-link` tool; `flip-link` flips the RAM layout so a stack overflow crashes into an unmapped region rather than silently corrupting BSS/statics
  - `runner = "probe-rs run --chip RP235x"` — enables `cargo run` and `cargo test --target thumbv8m.main-none-eabihf` to flash and run via probe-rs automatically (see Phase 7 Tier 3)
- [ ] Add a `[profile.release]` section to `Cargo.toml`:
  ```toml
  [profile.release]
  opt-level = "s"      # optimise for binary size
  lto = true           # link-time optimisation; reduces firmware size significantly
  codegen-units = 1    # required for LTO; disables parallel codegen
  debug = true         # retain symbols so probe-rs / defmt can decode RTT log messages
  ```
- [ ] Verify a `defmt`-based logging setup for debug output over RTT (probe-rs)
- [ ] Boot to a stable idle loop; confirm via `probe-rs` or UF2 flash
- [ ] Add `.vscode/settings.json` with `"rust-analyzer.cargo.target": "thumbv8m.main-none-eabihf"` and `"rust-analyzer.cargo.features": []` — without this, rust-analyzer analyzes the crate as a host binary and floods the IDE with false `no_std` errors
- [ ] Update `.devcontainer/devcontainer.json` with:
  - `"postCreateCommand": "rustup show"` — verifies the pinned toolchain is installed on container start
  - `"forwardPorts": [4242]` — exposes the TCP socket to the host for Tier 4 integration tests
  - `"customizations": { "vscode": { "settings": { "rust-analyzer.cargo.target": "thumbv8m.main-none-eabihf" } } }` — keeps rust-analyzer settings in one place alongside the extension list
- [ ] Create a minimal `.github/workflows/ci.yml` with two jobs — `lint` (`cargo fmt --check`) and `build` (`cargo build --release --target thumbv8m.main-none-eabihf`) — triggered on `push` and `pull_request`; expand to the full three-job pipeline in Phase 8c
- [ ] Expand `.gitignore` to also exclude `.env` (local credential overrides), `*.uf2` (build artefacts), and common editor/OS noise (`.DS_Store`, `*.swp`, `.idea/`)

**Key crates:**
- `embassy-rp` — async runtime, executor, and HAL for RP235x (provides all peripheral drivers; `rp235x-hal` is **not** used — the two HALs conflict)
- `embassy-executor` — task scheduling
- `defmt` + `defmt-rtt` — structured logging over RTT (not USB; probe-rs reads this via SWD)
- `panic-probe` — panic handler that logs via probe

---

### Phase 2 — Wi-Fi Connectivity (CYW43439)

**Goal:** Join a Wi-Fi network and open a TCP socket; fall back to provisioning if no credentials are stored.

- [ ] Integrate `cyw43` driver (CYW43439 chip on Pico W / Pico 2 W)
- [ ] Load the CYW43 firmware blob at link time
- [ ] Implement `embassy-net` network stack (IP + TCP)
- [ ] At startup, call `load_credentials()` (see Phase 6a) to read SSID/password from flash
  - [ ] If credentials found → proceed with STA join below
  - [ ] If no credentials found → enter provisioning mode (see Phase 6b–6d) before attempting STA join
- [ ] Perform DHCP lease on STA connect; expose IP over serial log
- [ ] Open a listening TCP socket on a configurable port (default `4242`)
- [ ] Implement a keep-alive / reconnect loop for dropped connections with the following explicit policy:
  - First retry: 5 s after disconnect; subsequent retries use exponential backoff doubling each attempt (5 s → 10 s → 20 s → 40 s) up to a maximum interval of 60 s
  - While retrying, emit `LedState::Reconnecting` (2 Hz medium blink)
  - After **10 minutes of continuous connection failure** (≈ 12+ retries), emit `LedState::Error` (SOS pattern) and stop retrying — the SOS pattern signals that manual intervention (power cycle or factory reset) is needed
  - On each retry attempt, log the attempt count and elapsed time via `defmt` (`warn!("reconnect attempt {} after {}s", attempt, elapsed_secs)`)

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
  "version": 1,
  "id": "abc123",
  "interface": "gpio",
  "action": "write",
  "pin": 15,
  "value": 1
}
```

**Response (Pico → client, success):**
```json
{
  "id": "abc123",
  "ok": true,
  "data": null,
  "error": null
}
```

**Response (Pico → client, failure):**
```json
{
  "id": "abc123",
  "ok": false,
  "data": null,
  "error": "invalid_pin"
}
```

> **Protocol versioning:** Commands with a `version` value other than `1` are rejected immediately with `{"id": "…", "ok": false, "error": "unsupported_version"}`. The field is required; commands missing it are rejected with `"error": "missing_version"`. This ensures clients get an actionable signal rather than silent failure when connecting to a mismatched firmware version.

> **Max message size:** The static receive buffer is **512 bytes**. Commands longer than 512 bytes (including the newline) are rejected with `"error": "msg_too_large"`. Clients must not send commands exceeding this limit.

#### `data` Field Schema for Read Operations

For write/set actions the `data` field is `null`. For read/transfer actions it carries the result:

| Interface + action | `data` value |
|--------------------|--------------|
| `gpio read` | `{"value": 0}` or `{"value": 1}` |
| `adc read` (channel 0–2) | `{"raw": 2048, "voltage": 1.650}` — raw is 12-bit (0–4095), voltage is float in V |
| `adc read` (temperature sensor) | `{"celsius": 27.3}` |
| `uart read` | `{"bytes": [72, 101, 108]}` — byte array (unsigned integers 0–255) |
| `spi transfer` | `{"bytes": [0xDE, 0xAD]}` — MISO bytes corresponding to MOSI bytes sent |
| `i2c read` / `i2c write_read` | `{"bytes": [0x0F, 0x42]}` |
| `config get` | `{"ssid": "MyNet", "ip": "192.168.1.42", "connected": true}` (password never included) |

> **Byte encoding:** byte arrays use JSON integer arrays (not base64) to keep the format `no_std`-friendly without a base64 codec dependency.

#### Interfaces & Actions

| Interface | Action | Required parameters |
|-----------|--------|---------------------|
| `gpio` | `read` | `{"pin": N}` |
| `gpio` | `write` | `{"pin": N, "value": 0\|1}` |
| `gpio` | `set_mode` | `{"pin": N, "mode": "input"\|"output", "pull": "up"\|"down"\|"none"}` |
| `uart` | `read` | `{"uart": 0\|1, "len": N}` — read up to N bytes |
| `uart` | `write` | `{"uart": 0\|1, "bytes": [0,…]}` |
| `uart` | `configure` | `{"uart": 0\|1, "baud": N, "data_bits": 7\|8, "parity": "none"\|"odd"\|"even", "stop_bits": 1\|2}` |
| `spi` | `transfer` | `{"spi": 0\|1, "bytes": [0,…]}` — MOSI bytes; MISO bytes returned in `data` |
| `spi` | `write` | `{"spi": 0\|1, "bytes": [0,…]}` — MOSI only; MISO discarded |
| `spi` | `configure` | `{"spi": 0\|1, "freq_hz": N, "cpol": 0\|1, "cpha": 0\|1}` |
| `i2c` | `read` | `{"i2c": 0\|1, "addr": N, "len": N}` |
| `i2c` | `write` | `{"i2c": 0\|1, "addr": N, "bytes": [0,…]}` |
| `i2c` | `write_read` | `{"i2c": 0\|1, "addr": N, "write_bytes": [0,…], "read_len": N}` |
| `i2c` | `configure` | `{"i2c": 0\|1, "freq_hz": N}` — 100000 or 400000 |
| `pwm` | `set_duty` | `{"channel": N, "duty_u16": 0–65535}` — raw 16-bit; `0` = always off, `65535` = always on |
| `pwm` | `set_freq` | `{"channel": N, "freq_hz": N}` |
| `pwm` | `enable` | `{"channel": N}` |
| `pwm` | `disable` | `{"channel": N}` |
| `adc` | `read` | `{"channel": 0\|1\|2\|"temp"}` — channels 0–2 map to GPIO26–28; `"temp"` is the onboard sensor |
| `usb` | `read` | `{"len": N}` |
| `usb` | `write` | `{"bytes": [0,…]}` |
| `config` | `get` | _(no extra parameters)_ |

- [ ] Define `Command` and `Response` structs including `version: u8`, `error: Option<&'static str>` fields as shown above
- [ ] Define `const MAX_MSG_LEN: usize = 512` and size all TCP receive buffers to this constant; document it in `PROTOCOL.md` and the Key Constraints table
- [ ] Implement a `no_std`-compatible JSON parser (using `serde-json-core`)
- [ ] Implement framing: newline-delimited records over the TCP stream; reject frames > `MAX_MSG_LEN` bytes
- [ ] Validate commands: reject unknown `version`, missing required fields, unknown `interface`, and return structured `error` codes on bad input

#### Error Code Catalogue

All error strings are `&'static str` values — no heap allocation. The complete set of valid `"error"` values is:

| Error code | Trigger condition |
|------------|-------------------|
| `"missing_version"` | `"version"` field absent from command |
| `"unsupported_version"` | `"version"` value is not `1` |
| `"msg_too_large"` | Frame length exceeds `MAX_MSG_LEN` (512 bytes) |
| `"malformed_json"` | JSON parse error |
| `"missing_field"` | A required parameter for the given action is absent |
| `"unknown_interface"` | `"interface"` value not in the Interfaces table |
| `"unknown_action"` | `"action"` value not valid for the given interface |
| `"invalid_pin"` | Pin number is out of range or reserved (e.g. CYW43 pins) |
| `"value_out_of_range"` | A numeric parameter exceeds its valid range |
| `"pin_in_use"` | Pin is already claimed by another peripheral |
| `"not_configured"` | Peripheral action called before `configure` |
| `"peripheral_busy"` | Peripheral is mid-transfer and cannot accept a new command |
| `"peripheral_error"` | HAL returned an error during the operation |

> **Stability contract:** error codes are part of the v1 protocol. New codes may be added in future versions; existing codes must not be renamed. Client libraries should handle unknown error codes gracefully (e.g. display the raw string) rather than treating them as fatal.

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
- [ ] **ADC**: read channels 0–2 (GPIO26–28) and the onboard temperature sensor; **GPIO29 is reserved** — on the Pico 2W it is the CYW43 SPI `DIO` line; ADC channel 3 / GPIO29 is physically unavailable to user code and must not be accessed
- [ ] **USB CDC**: read/write to the virtual serial port over USB

Each driver module exposes an `async fn handle(cmd: &Command) -> Response` function.
The router dispatches based on `cmd.interface` and awaits the result.

---

### Phase 5a — Async Task Model

**Goal:** Run networking and peripheral handling concurrently without blocking.

- [ ] Spawn an `embassy` task for the Wi-Fi driver (`cyw43` background task)
- [ ] Spawn a task for the `embassy-net` network stack
- [ ] Spawn a task for the TCP listener (accepts connections, reads framed messages)
- [ ] Spawn exactly one message-handling task per accepted connection; `TcpSocket::accept()` is **not** re-called until the current client disconnects (see single-connection constraint in Key Constraints and Non-Goals)
- [ ] Use `embassy::channel::Channel<CriticalSectionRawMutex, Command, 1>` for inter-task communication (command queue, response queue); capacity `N=1` is correct for a single-connection server and must be a named `const` rather than a magic number
- [ ] Ensure peripheral access is protected with `embassy::mutex::Mutex` where needed
- [ ] Enforce a **30-second read deadline** on each accepted TCP connection: if no complete framed message is received within 30 s of `accept()`, close the socket, log `warn!("TCP read timeout — closing idle connection")`, and re-enter `accept()`; use `embassy_time::with_timeout(Duration::from_secs(30), socket.read(…))` and map `TimeoutError` to a clean socket close

---

### Phase 5b — LED Status Signaling

**Goal:** Drive the CYW43 onboard LED to reflect device state at all times using the standard IoT single-LED blink-rate convention, recognizable to any user who has set up a home router or smart home device.

> **Hardware note:** On the Pico 2W the LED is wired to **CYW43 GPIO0**, not an RP2040 pin. It is driven exclusively via `cyw43.set_led(bool)` — no RP2040 HAL GPIO access is needed or possible.

#### LED State Reference

| # | `LedState` variant | Pattern name | Timing | Meaning |
|---|--------------------|--------------|--------|---------|
| 1 | `Booting` | 3-flash burst | 3 × (100 ms ON / 100 ms OFF), 1 s OFF, repeat | Firmware starting up |
| 2 | `Provisioning` | Slow blink, 1 Hz | 1 s ON / 1 s OFF | AP mode active, awaiting Wi-Fi setup |
| 3 | `Scanning` | Double-blink | 2 × (100 ms ON / 100 ms OFF), 700 ms OFF, repeat | Scanning for networks |
| 4 | `Connecting` | Fast blink, 5 Hz | 100 ms ON / 100 ms OFF | STA join / credential test in progress |
| 5 | `Connected` | Solid ON | Constant | Operational, TCP socket accepting |
| 6 | `Reconnecting` | Medium blink, 2 Hz | 250 ms ON / 250 ms OFF | Wi-Fi lost, retrying |
| 7 | `Error` | SOS (Morse) | · · · — — — · · · + 2 s pause, repeat | Unrecoverable error |
| 8 | `Saving` | 5 rapid flashes, then OFF | 5 × (100 ms ON / 100 ms OFF), then OFF | Saving credentials, rebooting |

*Solid ON = connected and SOS = error are universally recognized; the blink-rate scale (slow = idle/waiting → fast = busy) mirrors every consumer Wi-Fi router shipped in the past two decades.*

#### Implementation Checklist

- [ ] Create `src/led.rs`; define `pub enum LedState` with the 8 variants above
- [ ] Declare `pub static LED_SIGNAL: Signal<CriticalSectionRawMutex, LedState>` in `src/led.rs`
- [ ] Implement `const SOS_TIMING: &[(bool, u64)]` — 9 entries (ON/OFF pairs for 3 dits, 3 dahs, 3 dits) plus a trailing 2 s OFF pause
- [ ] Implement `#[embassy_executor::task] pub async fn led_task(runner: &'static CywRunner<'static>)`:
  - Loops: `loop { let state = LED_SIGNAL.wait().await; … }`
  - Each `LedState` arm drives `runner.set_led(bool)` + `Timer::after(Duration::from_millis(…))` in the correct pattern
  - The `Error`/SOS arm iterates over `SOS_TIMING` in a nested loop until a new signal arrives
- [ ] Spawn `led_task` from `main` as the **first** spawned task (before net/peripheral tasks)
- [ ] Emit `LED_SIGNAL.signal(LedState::Booting)` at the top of `main`, before any other work
- [ ] Wire all state transitions (see Phase 2 and Phase 6) to emit the corresponding `LedState`

**Key items already available:** `embassy_time::Timer`, `embassy_time::Duration`, `embassy_sync::signal::Signal` — all pulled in by Phase 5a dependencies.

---

### Phase 5c — Power Management

**Goal:** Reduce power consumption during idle periods (no active TCP client) without compromising server availability or Wi-Fi association stability.

> **Scope boundary:** RP2350 SLEEP and DORMANT modes are explicitly **excluded** from this firmware. DORMANT halts all oscillators, stops the CYW43 SPI bus entirely, and has no Wi-Fi-frame wakeup source — the device becomes unreachable to incoming TCP connections. This is a deliberate design decision documented here so future contributors know it was considered and ruled out, not overlooked. DORMANT/scheduled-wake is planned for consideration in a v2 variant.

#### Power Mode Compatibility Reference

| Mode | Total system current (CPU + Wi-Fi radio) | TCP server compatible? | Decision |
|------|------------------------------------------|------------------------|----------|
| Embassy executor WFI/WFE | ~35–45 mA | ✓ Automatic | Baseline — no work needed |
| CPU underclocking (48 MHz) | saves ~25% of CPU share | ✓ Yes | Use when no TCP client active |
| CYW43 PM2 beacon-period doze | saves ~10–15 mA | ✓ With guard | Enable idle only; disable on `accept()` |
| RP2350 SLEEP mode | ~3–5 mA | ✗ Misses SYN/ARP | **Excluded** |
| RP2350 DORMANT mode | ~0.7–3 mA | ✗ Stops CYW43 SPI | **Excluded** |

#### 5c-i — Embassy Executor WFI (baseline, no implementation work)

Embassy's async executor calls `cortex_m::asm::wfi()` automatically whenever all tasks are blocked on futures. The **CPU core** current drops to ~8–12 mA during genuine idle; however the total system draw remains ~35–45 mA because the CYW43 Wi-Fi radio continues running. Document this distinction as the power baseline; no checklist items required.

#### 5c-ii — CPU Frequency Scaling

> **⚠️ API validation required before implementation:** `ClockConfig::system_freq()` in `embassy-rp` is typically applied once at boot via `embassy_rp::init(config)`. Runtime dynamic frequency switching (called after `init()`) may require direct PAC register writes to `CLOCKS.clk_sys_div` and PLL control registers rather than a clean `ClockConfig` call. **Before implementing runtime clock switching, verify whether `embassy-rp` exposes a safe runtime frequency-change API on RP2350.** If no such API exists, the fallback is: boot at 48 MHz always (no runtime scaling); accept the ~25% CPU throughput reduction as the fixed cost of keeping always-on Wi-Fi availability.

- [ ] At firmware entry, initialize the RP2350 at **48 MHz** (XOSC direct, PLL bypassed) via `ClockConfig::system_freq(48_000_000)` — covers boot, provisioning, and idle TCP wait
- [ ] Scale up to **150 MHz** (PLL enabled) immediately when `TcpSocket::accept()` returns a connection — before reading any data
- [ ] Return to **48 MHz** when the TCP socket closes and the listener re-enters `accept()`
- [ ] Verify the CYW43 SPI clock divisor keeps the bus ≤ 33 MHz at both 48 MHz and 150 MHz system clock (adjust the SPI clock divider in `cyw43` init accordingly)

#### 5c-iii — CYW43 Wi-Fi Power Save (PM2)

- [ ] After a successful STA join, enable `PowerManagementMode::PowerSave` (PM2, DTIM1 interval) on the `cyw43::Control` handle — the Wi-Fi chip dozes between beacon windows, saving ~10–15 mA
- [ ] Disable PM2 (`PowerManagementMode::None`) immediately when `TcpSocket::accept()` succeeds, before reading any data — avoids latency spikes on the active connection
- [ ] Re-enable PM2 when the TCP socket is closed and the server is back in `accept()`
- [ ] Log all PM mode transitions via `defmt` (`info!("wifi pm: {}", mode)`) for debug builds
- [ ] Integrate with the Phase 2 keep-alive loop: if `embassy-net` signals the interface is down while PM2 is active, disable PM2 before the reconnect attempt and re-enable after a successful rejoin

> **Stability note:** CYW43 PM modes have documented cases of AP-side disassociation after prolonged idle. The guard (disable on `accept()`) and the keep-alive reconnect loop together mitigate this; however, if instability is observed in Tier 3 testing, PM2 can be disabled as a build-time feature flag.

#### 5c-iv — Peripheral Clock Gating

- [ ] In `main`, before spawning tasks, write `SLEEP_EN0` / `SLEEP_EN1` registers to disable clocks to peripheral blocks not yet initialized (UART, SPI1, I2C0/1, PWM, ADC)
- [ ] Each Phase 4 driver module enables its own peripheral clock in its init function before the first use
- [ ] Add a comment block in `main` mapping each `SLEEP_EN` bit to the peripheral it gates — this is a one-time static setup, not dynamic power management

**Key APIs:**
- `embassy_rp::clocks::ClockConfig::system_freq(hz)` — CPU frequency at init
- `cyw43::Control::set_power_management(PowerManagementMode)` — Wi-Fi PM mode
- `pac::CLOCKS.sleep_en0()` / `.sleep_en1()` — peripheral clock gating (raw PAC register access)

---

### Phase 6 — Configuration & Provisioning

**Goal:** Persist Wi-Fi credentials in flash and provide a captive-portal provisioning flow when no credentials are stored.

#### 6a — Flash Credential Storage

- [ ] Add `sequential-storage` + `embedded-storage` to `Cargo.toml` for wear-levelled flash key-value storage
- [ ] Define a `Credentials` struct: `ssid: heapless::String<32>`, `password: heapless::String<64>`
- [ ] Implement `load_credentials() -> Option<Credentials>` — reads from the last flash sector(s) at boot
- [ ] Implement `save_credentials(creds: &Credentials)` — called after a successful provisioning test
- [ ] Compile-time override: if `PICO_WIFI_SSID` and `PICO_WIFI_PASS` env vars are set at build time, use them and skip flash storage (development convenience only)
- [ ] Add `.env.example` at repo root with commented-out credential exports so developers know the exact variable names without reading this section:
  ```sh
  # Copy to .env, fill in your values, then run: source .env
  # .env is gitignored — never commit it
  # export PICO_WIFI_SSID="your-ssid"
  # export PICO_WIFI_PASS="your-password"
  ```

#### 6b — AP / Provisioning Mode

- [ ] If `load_credentials()` returns `None`, switch CYW43 into AP mode
- [ ] Broadcast an SSID derived at runtime from the CYW43 MAC address: `pico-setup-XXXX` where `XXXX` is the last 4 hex digits of the MAC (e.g. `pico-setup-A3F2`); format into a `heapless::String<20>` at boot — this ensures each device has a distinct SSID so users can distinguish multiple Pico 2W devices on the same network; a build-time prefix override via `env!("PICO_AP_SSID_PREFIX")` (default `"pico-setup"`) is also supported
- [ ] Assign static IP `192.168.4.1/24` to the AP interface
- [ ] Run a DHCP server via `embassy-net` to hand out `192.168.4.x` leases to connecting clients

#### 6c — Captive Portal HTTP Server

- [ ] Implement a minimal HTTP/1.0 server on port 80 (using `picoserve` or a hand-rolled TCP handler over `embassy-net`)
- [ ] `GET /` — trigger a CYW43 active scan; render a static HTML form listing discovered SSIDs as `<option>` elements plus a password `<input>` and a "Connect" submit button; include a "Hidden / other network" option that reveals a plain `<input type="text">` field for manual SSID entry — users on non-beaconing (hidden) networks must be able to type the SSID directly rather than relying solely on the scanned dropdown; all HTML is a `const` byte slice — no heap
- [ ] `POST /connect` — parse `application/x-www-form-urlencoded` body (fixed-size buffer) to extract `ssid` and `password` fields; forward to credential testing (6d)
- [ ] Scan results stored in a `heapless::Vec<heapless::String<32>, 16>` (up to 16 networks)
- [ ] Handle captive portal detection probes: iOS (`captive.apple.com`), Android (`connectivitycheck.gstatic.com`), Windows (`msftconnecttest.com`) all send HTTP GET requests to vendor-specific hosts; respond to any GET request whose `Host` header is not `192.168.4.1` with `302 Found` redirecting to `http://192.168.4.1/` — this causes the OS to automatically open the setup page rather than silently failing the portal check

#### 6d — Credential Testing & Saving

- [ ] On `POST /connect`: temporarily switch CYW43 from AP to STA mode and attempt to join the submitted network with a configurable timeout
- [ ] **Success** (ordered — confirmation must reach the user before the device reboots):
  1. Serve a `200 OK` HTML confirmation page containing:
     - A "Connected!" success banner
     - The SSID that was successfully joined
     - The DHCP-assigned IP address the device received
     - A visible countdown message ("Saving settings and restarting in 5 s…") via `<meta http-equiv="refresh" content="5">` so the user can read the result before the page disappears
  2. Flush the complete HTTP response to the client (drain TCP send buffer) before proceeding
  3. Call `save_credentials()` to write SSID + password to flash
     - **If `save_credentials()` fails** (flash write error): do **not** arm the watchdog; serve an HTML error page explaining the storage failure with a "Try again" link; return CYW43 to AP mode and re-enter the portal loop
  4. Arm the hardware watchdog with a ~5 s timeout to trigger a soft reboot; device restarts into STA mode (Phase 2)
- [ ] **Failure**: serve an HTML error page that includes the reason (e.g. "Wrong password", "Network not found", "Timed out") and a "Try again" link → return CYW43 to AP mode and re-enter the portal loop

#### 6e — Config Reporting

- [ ] Expose a `GET /status` HTTP endpoint (portal only, while in AP mode) that returns JSON `{"ssid": "...", "connected": false}`
- [ ] After STA connection, respond to a special TCP command (`{"interface":"config","action":"get"}`) with `{"ssid":"...","ip":"...","connected":true}` (password never included)

#### 6f — Factory Reset

- [ ] On boot, check if the BOOTSEL button (GPIO23 on RP2350, active low) is held continuously for 5 s after power-on; if so, signal `LedState::Saving`, erase the credentials flash sector, log `warn!("factory reset triggered via BOOTSEL hold")`, and resume normal boot — which will detect no credentials and enter AP mode
- [ ] Document the factory reset procedure in `README.md` (end-user section) and `Troubleshooting.md` (wiki): "Hold BOOTSEL for 5 s on power-up to erase stored Wi-Fi credentials and return to provisioning mode"

> **Why this matters:** without a physical factory reset, a user with corrupted credentials (not blank, not valid) has no recovery path short of reflashing the firmware. The 5-second hold time prevents accidental triggers during normal plug-in.

**New key crates:**
- `sequential-storage` — wear-levelled key-value store in flash
- `embedded-storage` — flash read/write traits (`NorFlash`)
- `picoserve` — `no_std` async HTTP/1.x server for `embassy-net` (or hand-rolled alternative)

---

### Phase 7 — Testing & Validation

> **Hardware prerequisites for Tier 3 & 4 tests:**
> - Raspberry Pi Pico **2W** (not Pico 1 or plain Pico 2)
> - Data-capable USB-A → micro-USB cable
> - SWD debug probe (e.g. [Raspberry Pi Debug Probe](https://www.raspberrypi.com/products/debug-probe/) or a second Pico flashed as [Picoprobe](https://github.com/raspberrypi/picoprobe))
> - Jumper wires (for UART TX→RX loopback and I2C pull-up resistors)
> - A 2.4 GHz 802.11n access point the device can reach
>
> Tier 1 and Tier 2 tests run entirely on the host machine — no hardware or probe is needed.

Testing is structured in four tiers, each runnable independently. A clean `cargo build` or `cargo build --release` **never** includes test code or mock peripherals — all test isolation is enforced by Rust's standard `#[cfg(test)]` and `[dev-dependencies]` mechanisms.

#### Tier 1 — Host Unit Tests (no hardware required)

Compile and run protocol and routing logic on the host machine using the standard Rust test harness.

```sh
cargo test --target x86_64-unknown-linux-gnu
```

- Tests message parsing, JSON framing, command validation, and routing dispatch
- All test modules are gated with `#[cfg(test)]` and excluded from firmware builds
- No `no_std` constraint in test context — host tests use `std`

> **Test placement rule:** Rust files under `tests/` are *integration tests* — they compile as separate crates and can only access `pub` API. Tests that exercise private or `pub(crate)` implementation details (parsers, serializers, internal state machines) **must** live as `#[cfg(test)]` modules inside the relevant source file (`protocol.rs`, `led.rs`, `provisioning/storage.rs`, `provisioning/portal.rs`). Only use `tests/host/` for integration-style host tests that exercise the module's public surface. This distinction applies to all Tier 1 and Tier 2 test items below.

- [ ] Write `#[test]` cases for `Command` / `Response` serialization and deserialization
- [ ] Write `#[test]` cases for framing edge cases (partial reads, oversized messages, malformed JSON)
- [ ] Write `#[test]` cases for router dispatch (correct handler called for each `interface` value)
- [ ] Test that the `LedState` enum has exactly 8 variants — one for each row in the Phase 5b reference table (prevents silent omissions)
- [ ] Test that `SOS_TIMING` has exactly 9 ON/OFF pairs in the correct dit/dah/dit order and alternates correctly between `true` (ON) and `false` (OFF)
- [ ] Write `#[test]` cases for `load_credentials()` returning `None` on a blank (0xFF) flash buffer
- [ ] Write `#[test]` cases for `save_credentials()` / `load_credentials()` round-trip
- [ ] Write `#[test]` cases for the HTTP request line parser: valid `GET /`, `POST /connect`, and malformed inputs
- [ ] Write `#[test]` cases for URL-encoded body parsing (`ssid=MyNet&password=secret`), including percent-encoded characters
- [ ] Write `#[test]` cases for protocol versioning: a command missing `"version"` returns `"error": "missing_version"`; a command with `"version": 2` returns `"error": "unsupported_version"`
- [ ] Write a `#[test]` case for oversized frame rejection: a frame of exactly 513 bytes returns `"error": "msg_too_large"`; a frame of exactly 512 bytes is accepted
- [ ] Write `#[test]` cases verifying each error code in the Error Code Catalogue is returned for its specific trigger condition (one test per error code)
- [ ] Write `#[test]` cases for the `data` field contents of all read-type operations: GPIO read, ADC channel read, temperature read, UART read, SPI transfer, I2C read — verify the field shape matches the `data` field schema table in Phase 3

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
- [ ] Measure current draw with a USB power meter or current probe in three states: (a) booting at 48 MHz before Wi-Fi join, (b) idle STA at 48 MHz with PM2 active and no TCP client, (c) active at 150 MHz with PM0 and a TCP client connected — confirm (b) < (c) by at least 10 mA
- [ ] Hold the device idle in PM2 mode for 30 minutes; confirm it remains reachable (ping, then TCP connect) with no manual intervention
- [ ] Verify via `defmt` log that CPU frequency transitions to 150 MHz on `accept()` and back to 48 MHz on socket close
- [ ] Confirm LED shows solid ON within 30 s of the device reaching connected + operational state
- [ ] Confirm LED shows slow blink (1 Hz) when device boots without stored credentials (AP mode active)
- [ ] Force an unrecoverable error (e.g. corrupt flash) and confirm the SOS pattern is emitted on the LED
- [ ] Erase credentials flash sector; verify device boots into AP mode and `pico-setup` SSID is visible to a nearby device
- [ ] Connect to `192.168.4.1` and confirm `GET /` returns an HTML page containing at least one `<option>` element (from SSID scan)
- [ ] Submit valid credentials via `POST /connect`; confirm device reboots and re-appears in STA mode
- [ ] Factory reset test: with valid credentials stored in flash, hold BOOTSEL for 5 s during power-on; verify via `defmt` log that `"factory reset triggered via BOOTSEL hold"` is emitted; verify the `pico-setup-XXXX` SSID becomes visible to a nearby device within 30 s

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
- [ ] Simulate factory reset (erase credentials flash sector); confirm provisioning mode activates and `pico-setup` SSID appears
- [ ] Submit valid SSID + password via `POST /connect` to the portal; confirm device saves credentials, reboots, connects to the target network, and the TCP socket on port 4242 becomes reachable
- [ ] Submit invalid credentials via `POST /connect`; confirm device returns an error page and remains in AP mode
- [ ] Protocol version mismatch: send `{"version":2,"id":"1","interface":"gpio","action":"read","pin":0}\n`; verify response is `{"id":"1","ok":false,"data":null,"error":"unsupported_version"}`
- [ ] Oversized message: send a 513-byte frame; verify response is `{"id":"…","ok":false,"data":null,"error":"msg_too_large"}` and the connection remains open for subsequent commands
- [ ] Error code stability: for each error code in the Error Code Catalogue, send a command that triggers it and assert the exact error string in the response — this test is the regression guard that prevents error codes from being silently renamed between firmware versions

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

#### 8 — Core Packaging & Distribution

- [ ] Build script that produces a `.uf2` file flashable via USB drag-and-drop
- [ ] Document the wire protocol and all commands in a `PROTOCOL.md`
- [ ] Write `LED_STATUS.md` — end-user LED blink-code reference containing:
  - The full 8-state reference table (state name, pattern name, timing, meaning)
  - An ASCII timing diagram for each pattern so the rhythm is visually obvious
  - A "what does the LED mean right now?" quick-diagnosis section
- [ ] Before adding the `client/` crate, convert the root `Cargo.toml` from a single-crate manifest to a workspace manifest:
  ```toml
  [workspace]
  members = [".", "client"]
  resolver = "2"
  ```
  This must be done as its own commit before the `client/` directory is created — a non-workspace `Cargo.toml` with a `client/` sibling crate will fail to build
- [ ] Provide a reference client library in Rust as a `client/` workspace crate named `pico-socketeer-client`; publish to crates.io at v1.0 alongside the firmware release
- [ ] CI pipeline (GitHub Actions) that:
  - Builds the firmware (`cargo build --release --target thumbv8m.main-none-eabihf`)
  - Runs Tier 1 & 2 host tests (`cargo test --target x86_64-unknown-linux-gnu`)
  - Produces a `.uf2` artifact on each tagged release

#### 8a — End-User Documentation

- [ ] Write `README.md` with three clearly separated zones:
  - **Quick-start (end user):** hardware prerequisites (Pico 2W, data-capable USB-A cable, 2.4 GHz AP), flash method (UF2 drag-and-drop *or* `probe-rs run`), first-boot LED behaviour reference (link to `LED_STATUS.md`), provisioning walkthrough (connect to `pico-setup` SSID → open `192.168.4.1` → enter credentials → wait for confirmation page → device reboots into STA mode)
  - **Developer setup:** one-liner to open the devcontainer, the four key commands (build, flash via probe-rs, flash via UF2, run host tests), link to `CONTRIBUTING.md`
  - **Project links table:** `PROTOCOL.md`, `LED_STATUS.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, GitHub Releases page
- [ ] Add a "Hardware you need" callout to `README.md`: Pico **2W** specifically (not Pico 1 or plain Pico 2), data-capable USB-A → micro-USB cable, 2.4 GHz 802.11n AP, host machine with USB port (no drivers needed for UF2 drag-and-drop)
- [ ] Add a "Send your first command" code block to `README.md` with both a `nc` one-liner and a minimal Python `socket` snippet sending `{"version":1,"id":"1","interface":"gpio","action":"write","pin":15,"value":1}\n` to port 4242 and printing the response; note `PICO_IP` must be substituted with the IP shown in the provisioning confirmation page; the `"version":1` field is required — omitting it causes the firmware to reject the command with `"error": "missing_version"`

#### 8b — Developer Documentation and Tooling Enforcement

- [ ] Write `CONTRIBUTING.md` covering:
  - Prerequisites: VS Code + Dev Containers extension (or `docker` CLI) — all other tools are inside the devcontainer
  - Devcontainer quick-start: `Dev Containers: Reopen in Container` or `devcontainer up --workspace-folder .`
  - Build: `cargo build --release`
  - Flash (probe-rs): `probe-rs run --chip RP235x target/thumbv8m.main-none-eabihf/release/pico-socketeer`
  - Flash (UF2): hold BOOTSEL, plug USB, run `elf2uf2-rs target/thumbv8m.main-none-eabihf/release/pico-socketeer`
  - Host unit tests: `cargo test --target x86_64-unknown-linux-gnu`
  - Lint: `cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings`
  - Format check: `cargo fmt --check`
  - Code style: `rustfmt` defaults; `clippy` at `deny(warnings)`; imperative-mood commit messages ≤ 72 chars
  - Branching model: feature branches off `master`, PRs required, no direct pushes to `master`
  - Branch protection note (must be set manually in GitHub Settings): require `lint` and `build` status checks to pass, require one approving review, disallow direct pushes to `master`
  - Release process (see 8c)
- [ ] Add `rustfmt.toml` at repo root with `edition = "2024"` set explicitly; add a comment that all other settings are left at `rustfmt` defaults (prevents toolchain-version drift)
- [ ] Add `#![deny(clippy::all)]` to `src/main.rs`; note in `CONTRIBUTING.md` that all future source files carry the same attribute at crate root
- [ ] Add `CLAUDE.md` at repo root documenting project conventions for AI-assisted development (the devcontainer installs `Anthropic.claude-code`):
  - **No heap:** `no_std` + `no alloc`; all buffers use `heapless` — never suggest `Vec`, `String`, `Box`, or `format!()`
  - **Async runtime:** `embassy` executor only; never suggest `std::thread`, `tokio`, or `async-std`
  - **Logging:** `defmt` macros (`info!`, `warn!`, `error!`) — never `println!` or `eprintln!`
  - **Error codes:** all `"error"` values are `&'static str` — no heap allocation for error messages
  - **Single HAL:** `embassy-rp` provides all peripheral access; never suggest `rp235x-hal` (the two conflict)
  - **Commit style:** imperative mood, ≤ 72 characters, no period at end (e.g. `Add GPIO read support`)

#### 8c — GitHub Project Health

- [ ] Extend `.github/workflows/ci.yml` with a `pull_request` trigger targeting `master`, structured as three jobs:
  1. `lint`: `cargo fmt --check` then `cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings`
  2. `build`: `needs: [lint]`; `cargo build --release --target thumbv8m.main-none-eabihf` then `cargo test --target x86_64-unknown-linux-gnu` (Tier 1 & 2)
  3. `release`: `needs: [build]`, gated on `github.ref_type == 'tag'`; runs `elf2uf2-rs` and uploads `.uf2` as a GitHub Release asset
  Add a comment in `ci.yml` that Tier 3 and Tier 4 tests require physical hardware and are run manually before tagging a release; runner: `ubuntu-latest` for all jobs
- [ ] Write `.github/ISSUE_TEMPLATE/bug_report.md` with fields: firmware version (GitHub Releases tag), hardware (Pico 2W confirmed? clone board?), host OS, reproduction steps, LED state at time of failure (reference to `LED_STATUS.md`), expected vs actual behaviour, `defmt` RTT log output (if available via probe-rs)
- [ ] Write `.github/ISSUE_TEMPLATE/feature_request.md` with fields: problem statement, proposed interface / protocol change (with example JSON command/response if applicable), affected OBJECTIVE.md phase(s), whether the requester is willing to implement it
- [ ] Write `.github/PULL_REQUEST_TEMPLATE.md` with a checklist:
  - `cargo fmt --check` passes
  - `cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings` passes
  - `cargo test --target x86_64-unknown-linux-gnu` passes (Tier 1 & 2)
  - OBJECTIVE.md phase checkboxes updated if this PR completes a planned item
  - `CHANGELOG.md` `[Unreleased]` section updated with a one-line entry
  - For protocol changes: `PROTOCOL.md` updated
  - For LED behaviour changes: `LED_STATUS.md` updated
  - For new end-user-visible behaviour: `README.md` quick-start section reviewed
- [ ] Create `CHANGELOG.md` following [Keep a Changelog](https://keepachangelog.com) format (`[Unreleased]` / `[x.y.z] - YYYY-MM-DD` with `Added`, `Changed`, `Fixed`, `Removed` sub-sections); add initial `[Unreleased]` stub; document in `CONTRIBUTING.md` that every PR touching user-visible behaviour must add a line under `[Unreleased]`
- [ ] Add a "Release process" section to `CONTRIBUTING.md`: (1) move `[Unreleased]` entries to a new versioned section in `CHANGELOG.md`; (2) bump `version` in `Cargo.toml`; (3) run Tier 1 & 2 tests locally; (4) run Tier 3 & 4 on hardware; (5) push `vx.y.z` tag to trigger CI `release` job; (6) copy CHANGELOG section into the GitHub Release description

#### 8d — GitHub Wiki (git submodule)

> **Decision boundary:** Only standalone reference material that evolves independently of the firmware belongs in the wiki. Docs that must stay in sync with code changes (`PROTOCOL.md`, `LED_STATUS.md`, `CONTRIBUTING.md`, `CHANGELOG.md`) remain in the repo and are reviewed in PRs.

- [ ] Add the GitHub Wiki as a git submodule: `git submodule add https://github.com/<owner>/pico-socketeer.wiki.git wiki`
- [ ] Populate `wiki/` with standalone reference pages:
  - `Home.md` — wiki landing page; mirrors the project links table from `README.md` plus links to the wiki sub-pages
  - `Troubleshooting.md` — common failure modes keyed to LED state (e.g. "LED shows SOS — what now?"), with `defmt` log snippets and resolution steps
  - `Hardware-Wiring.md` — full GPIO pin assignment table (which pins are reserved for CYW43 vs. available for user peripherals), recommended test wiring (UART loopback, I2C pull-ups, ADC voltage divider)
  - `Tutorials.md` — step-by-step examples: blink an external LED via GPIO command, read a temperature sensor via ADC command, drive a servo via PWM command
  - `Architecture.md` — long-form async task model explanation, embassy executor internals, inter-task channel topology (complements the ASCII diagram in this file)
  - `Known-Issues.md` — per-release errata, CYW43 PM2 stability notes, RP235x silicon errata references
- [ ] Add a `wiki` CI job in `.github/workflows/ci.yml` that runs `git submodule update --init` and validates all wiki Markdown files with `markdownlint`; this job runs on PR but does not push — wiki pushes are performed manually or via a dedicated `on: push to master` workflow
- [ ] Add a note to `CONTRIBUTING.md` explaining the wiki split: docs that must be reviewed alongside code changes live in the repo; standalone reference material lives in `wiki/`; to edit locally: `git submodule update --init && cd wiki`, then edit and `git push` from inside the submodule
- [ ] Update `dependabot.yml` to add `package-ecosystem: github-actions` monitoring `.github/workflows/` (weekly schedule) so CI action versions are kept current alongside Cargo and devcontainer dependencies

---

## Directory Structure (target)

```
pico-socketeer/
├── src/
│   ├── main.rs          # Entry point, executor, task spawning
│   ├── net.rs           # Wi-Fi init, TCP listener (STA mode)
│   ├── led.rs           # LedState enum, SOS_TIMING const, led_task, LED_SIGNAL
│   ├── protocol.rs      # Command / Response types, JSON framing
│   ├── router.rs        # Dispatch commands to interface handlers
│   ├── provisioning/
│   │   ├── mod.rs       # Provisioning mode entry point (AP setup, portal loop)
│   │   ├── storage.rs   # load_credentials() / save_credentials() via flash
│   │   └── portal.rs    # HTTP endpoint handlers + static HTML byte slices
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
│   │   └── protocol_tests.rs   # Tier 1 & 2: integration-style host tests (public API only); private-item unit tests live as #[cfg(test)] modules inside each source file
│   └── integration/
│       └── tcp_client.rs       # Tier 4: TCP integration test (PICO_IP=... cargo test --test integration)
├── memory.x             # Linker script for RP235x
├── build.rs             # Build script (links memory.x, embeds CYW43 firmware)
├── Cargo.toml           # Workspace manifest after Phase 8 conversion (members = [".", "client"])
├── rust-toolchain.toml  # Pins Rust stable channel, rustfmt/clippy/rust-src, thumbv8m target (Phase 1)
├── rustfmt.toml         # edition = "2024"; all other settings at rustfmt defaults (Phase 8b)
├── CLAUDE.md            # AI coding assistant conventions: no-heap, embassy, defmt, error codes (Phase 8b)
├── .env.example         # Documents PICO_WIFI_SSID / PICO_WIFI_PASS compile-time overrides (Phase 6a)
├── client/              # pico-socketeer-client workspace crate (Phase 8)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs       # Reference client library; published to crates.io at v1.0
├── .vscode/
│   ├── launch.json      # LLDB debug configuration
│   └── settings.json    # rust-analyzer target + features override for embedded (Phase 1)
├── OBJECTIVE.md         # This file
├── PROTOCOL.md          # Wire protocol reference (Phase 8)
├── LED_STATUS.md        # LED blink-code reference for end users (Phase 8)
├── README.md            # Quick-start (end user), developer setup, project links (Phase 8a)
├── CONTRIBUTING.md      # Devcontainer setup, build/test/flash commands, release process (Phase 8b)
├── CHANGELOG.md         # Keep a Changelog format; [Unreleased] accumulates each PR (Phase 8c)
├── wiki/                # git submodule → pico-socketeer.wiki.git (Phase 8d)
│   ├── Home.md          # Wiki landing page
│   ├── Troubleshooting.md
│   ├── Hardware-Wiring.md
│   ├── Tutorials.md
│   ├── Architecture.md
│   └── Known-Issues.md
└── .github/
    ├── dependabot.yml
    ├── workflows/
    │   └── ci.yml       # Phase 1: minimal lint + build; expanded to lint → build → release → wiki in Phase 8c/8d
    ├── ISSUE_TEMPLATE/
    │   ├── bug_report.md
    │   └── feature_request.md
    └── PULL_REQUEST_TEMPLATE.md
```

---

## Non-Goals (v1)

- TLS/encrypted transport (planned for v2)
- Authentication or authorization — any host on the LAN can connect to port 4242 and control all peripherals; access control is planned alongside TLS in v2
- Credential encryption at rest — SSID and password are stored in plaintext flash; encryption at rest is planned for v2
- Binary protocol framing — JSON-only in v1; a compact binary format is planned for v2 to reduce message size on bandwidth-constrained links
- Multiple concurrent TCP clients — v1 is a single-connection server; additional `accept()` calls are deferred until the active client disconnects
- Over-the-air firmware updates
- Multi-device mesh networking
- A general-purpose web UI or REST API (TCP sockets only in v1) — **except** the one-time provisioning captive portal, which is a lightweight HTTP/1.0 server active only when no credentials are stored in flash
- RP2350 SLEEP or DORMANT low-power modes — incompatible with an always-on TCP socket server; no Wi-Fi-frame wakeup source exists from either mode (planned for a v2 scheduled-wake variant where the device wakes periodically to handle requests)
- Bluetooth (BLE or Classic) — the CYW43439 chip supports Bluetooth 5.2 (LE Central/Peripheral + Classic), but the `cyw43` Rust driver's BT stack path is less mature than its Wi-Fi path and is out of scope for v1; planned for consideration in v2
- PIO (Programmable I/O) — RP2350 has 3 PIO blocks / 12 state machines capable of emulating custom serial protocols, WS2812 LEDs, SD card, VGA, and more; PIO programs are inherently use-case-specific and incompatible with a generic peripheral bridge interface; planned for consideration in a future specialist variant
- HSTX peripheral — the RP2350 HSTX high-speed serial transmit block (designed for DVI/HDMI output and similar streaming use cases) is not relevant to the network-bridge goal and is excluded from v1

---

## Key Constraints

| Constraint | Detail |
|------------|--------|
| No heap | `no_std` + `no alloc`; all buffers are statically sized with `heapless` |
| Single core | RP235x is dual Cortex-M33; v1 uses core 0 only via `embassy` |
| Core architecture | Cortex-M33 (`thumbv8m.main-none-eabihf`); RP2350 also offers dual RISC-V Hazard3 cores, but `embassy-rp` does not yet support the RISC-V RP2350 target — Cortex-M33 is the only viable choice for this firmware |
| Flash size | 4 MB on Pico 2; CYW43 firmware blob is ~230 KB |
| RAM | 520 KB SRAM; message buffers must be conservatively sized |
| Async only | All I/O is non-blocking via `embassy`; no RTOS threads |
| Max message size | Static TCP receive buffer of **512 bytes**; commands or responses exceeding this are rejected with `"error": "msg_too_large"` |
| TCP clients | Single concurrent client in v1; additional `accept()` calls are deferred until the active client disconnects |
| Power | RP2350 SLEEP/DORMANT are incompatible with always-on TCP server (no Wi-Fi frame wakeup source); power saving is limited to CPU underclocking (48 MHz idle / 150 MHz active) + CYW43 PM2 (idle-only guard) + Embassy executor WFI |
