# pico-conduit

[![CI](https://github.com/yogaxpto/pico-conduit/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/yogaxpto/pico-conduit/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![Rust](https://img.shields.io/badge/Rust-stable%20%E2%89%A5%201.85-orange.svg)](https://www.rust-lang.org/)

Rust firmware for the **Raspberry Pi Pico 2W** (RP2350 / CYW43439) and **Pico W** (RP2040 /
CYW43439) that exposes a JSON command/response protocol over one of three compile-time
selectable transports — **TCP** (default), **WebSocket**, or **MQTT** — allowing a remote
client to read and write GPIO, UART, SPI, I2C, PWM, ADC, and USB peripherals.

## Hardware You Need

> **Required hardware**
> - Raspberry Pi **Pico 2W** (RP2350 + CYW43439) or **Pico W** (RP2040 + CYW43439)
> - Data-capable USB-A → micro-USB cable (charge-only cables will not enumerate)
> - 2.4 GHz 802.11n Wi-Fi access point (5 GHz not supported by CYW43439)
> - Host machine with a USB port — no drivers needed for UF2 drag-and-drop

## Quick Start (End User)

### 1. Flash the firmware

Download the latest `pico-conduit.uf2` from [Releases](../../releases).

Hold the BOOTSEL button on the Pico 2W while plugging in USB. Drag-and-drop the `.uf2` file
onto the `RPI-RP2` drive that appears. The board reboots automatically.

### 2. Provision Wi-Fi

On first boot (no credentials stored), the LED blinks slowly (1 s on / 1 s off — provisioning
mode). Connect to the `pico-setup-XXXX` Wi-Fi access point (password: `picosetup`) and navigate
to `http://192.168.4.1` to enter your Wi-Fi credentials. The board saves them and reboots into
station mode.

To reset credentials, hold the BOOTSEL button for ≥ 5 seconds while powering on.

### 3. Send your first command

Once the LED is solid ON (connected), connect to port 4242 with `nc`:

```sh
nc <pico-ip-address> 4242
{"version":1,"id":"1","interface":"gpio","action":"write","pin":15,"value":1}
```

Or use Python:

```python
import socket, json

HOST = "PICO_IP"  # substitute the IP shown in the RTT log during provisioning
cmd  = {"version": 1, "id": "1", "interface": "gpio", "action": "write",
        "pin": 15, "value": 1}
with socket.create_connection((HOST, 4242)) as s:
    s.sendall((json.dumps(cmd) + "\n").encode())
    print(s.makefile().readline())
```

> **Note:** `"version": 1` is required in every command. Omitting it causes the firmware
> to reject the command with `"error": "missing_version"`.

### Pipelining

The server reads and responds to commands sequentially. Clients may send multiple
newline-delimited commands without waiting for each response — responses are emitted in
the same order as commands were received.

```sh
# Send 3 commands in a single burst; responses arrive in order
printf '{"version":1,"id":"1","interface":"gpio","action":"set_mode","pin":0,"mode":"output"}\n{"version":1,"id":"2","interface":"gpio","action":"write","pin":0,"value":1}\n{"version":1,"id":"3","interface":"system","action":"get_version"}\n' \
  | nc <pico-ip-address> 4242
```

A malformed command returns an error response for that command and processing continues
for subsequent commands — the connection is never dropped on a protocol error.

See [PROTOCOL.md](PROTOCOL.md) for the full command reference.

## Developer Setup

> **Quick start with Dev Container:** Open this project in [VS Code](https://code.visualstudio.com/) or
> [GitHub Codespaces](https://github.com/features/codespaces) — the included Dev Container has all tools
> pre-installed (Rust, embedded targets, probe-rs, flip-link, rust-analyzer). See
> [.devcontainer/](.devcontainer/) for details.

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | ≥ 1.85 | `rustup toolchain install stable` |
| Embedded target (Pico 2W) | thumbv8m.main-none-eabihf | `rustup target add thumbv8m.main-none-eabihf` |
| Embedded target (Pico W) | thumbv6m-none-eabi | `rustup target add thumbv6m-none-eabi` |
| flip-link | latest | `cargo install flip-link` |
| probe-rs | latest | `cargo install probe-rs-tools` |

### Build

```sh
# Clone and enter the repo
git clone https://github.com/<your-org>/pico-conduit.git
cd pico-conduit

# Download CYW43 firmware blobs (one-time setup)
mkdir cyw43-firmware
curl -fsSL https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware/43439A0.bin \
     -o cyw43-firmware/43439A0.bin
curl -fsSL https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware/43439A0_clm.bin \
     -o cyw43-firmware/43439A0_clm.bin

# Build firmware (Pico 2W — default)
make build

# Build firmware (Pico W)
make build BOARD=pico1w
```

Or directly with cargo:

```sh
# Pico 2W (default: TCP on port 4242)
cargo build --release --target thumbv8m.main-none-eabihf

# Pico W
cargo build --release --target thumbv6m-none-eabi --no-default-features --features embedded,pico1w,transport-tcp
```

### Transport Feature Flags

Exactly one transport must be enabled per build. The features are mutually exclusive.

| Transport | Feature | Port/Broker | Build command (Pico 2W) |
|-----------|---------|-------------|------------------------|
| TCP (default) | `transport-tcp` | 4242 | `cargo build --release` |
| WebSocket | `transport-websocket` | 4243 | `cargo build --release --no-default-features --features embedded,pico2w,transport-websocket` |
| MQTT | `transport-mqtt` | broker (default 1883) | `cargo build --release --no-default-features --features embedded,pico2w,transport-mqtt` |

For Pico W builds, replace `pico2w` with `pico1w` and use target `thumbv6m-none-eabi`.

**MQTT** requires an external MQTT broker. The broker host and port are configured via the
provisioning portal. The device subscribes to `pico/<mac>/cmd` and publishes responses to
`pico/<mac>/resp` (where `<mac>` is the last 4 hex digits of the MAC address).

**WebSocket** uses standard RFC 6455 text frames. Connect with any WebSocket client
(e.g. `websocat ws://<pico-ip>:4243`).

### Flash (probe-rs)

Connect a debug probe (e.g. another Pico running picoprobe) and run:

```sh
# Pico 2W
cargo run --release --target thumbv8m.main-none-eabihf

# Pico W
cargo run --release --target thumbv6m-none-eabi --no-default-features --features embedded,pico1w
```

The `.cargo/config.toml` runner is `probe-rs run --chip RP235x` (Pico 2W) or
`probe-rs run --chip RP2040` (Pico W).

### Run Host Tests (Tier 1 + 2)

No hardware required — tests run against the host triple:

```sh
cargo test --test host --no-default-features --target aarch64-unknown-linux-musl
```

### Lint

```sh
cargo fmt --check
cargo clippy --target thumbv8m.main-none-eabihf -- -D warnings
```

### Compile-time Wi-Fi Credentials (dev convenience)

```sh
PICO_WIFI_SSID=MyNetwork PICO_WIFI_PASS=secret cargo run --release --target thumbv8m.main-none-eabihf
```

## Architecture

```
src/
├── main.rs               # Embassy entry point — spawns tasks
├── net.rs                # CYW43 init, transport servers, LED driver (embedded-only)
├── lib.rs                # Library root (host + embedded)
├── board.rs              # Board-specific constants (flash size, chip ID)
├── protocol.rs           # JSON serialisation / deserialisation, framing
├── router.rs             # Command dispatcher (interface + action validation)
├── transport.rs          # Transport trait (async read/write abstraction)
├── ws.rs                 # WebSocket framing, SHA-1, accept key (no_std)
├── mqtt.rs               # MQTT topic/client-id helpers, backoff (no_std)
├── led.rs                # LED state machine constants
├── interfaces/           # Per-peripheral handlers
│   ├── gpio.rs
│   ├── uart.rs
│   ├── spi.rs
│   ├── i2c.rs
│   ├── pwm.rs
│   ├── adc.rs
│   └── usb.rs
└── provisioning/         # Flash credential storage + captive portal
    ├── mod.rs
    ├── storage.rs
    └── portal.rs
```

## Updating Firmware

### Step 1 — Send the reboot command

With the device running and connected to Wi-Fi, send:

```sh
echo '{"version":1,"id":"1","interface":"system","action":"reboot_to_bootloader"}' \
    | nc 192.168.1.x 4242
```

The device replies `{"id":"1","ok":true,...}`, flashes the LED 10 times rapidly, then
reboots into USB bootloader mode.

### Step 2 — Flash the new firmware

1. Plug a data-capable USB-A → micro-USB cable from the Pico 2W to your computer.
2. A drive named **RPI-RP2** appears on your computer.
3. Drag the new `.uf2` file onto the drive.
4. The device reboots automatically and resumes normal operation.

### Manual fallback

If the device is unreachable over TCP (e.g. during initial setup or after a failed flash):

1. Hold the **BOOTSEL** button on the Pico 2W.
2. While holding BOOTSEL, plug in the USB cable.
3. Release BOOTSEL — the **RPI-RP2** drive appears.
4. Proceed from step 3 above.

---

## Related Documents

- [PROTOCOL.md](PROTOCOL.md) — Full wire protocol specification
- [LED_STATUS.md](LED_STATUS.md) — LED blink pattern reference
- [CONTRIBUTING.md](CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](CHANGELOG.md) — Release history

## License

MIT — see [LICENSE](LICENSE).
