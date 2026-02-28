# pico-socketeer

Rust firmware for the **Raspberry Pi Pico 2W** (RP2350 / CYW43439) that exposes a JSON-over-TCP
socket server on port **4242**, allowing a remote client to read and write GPIO, UART, SPI, I2C,
PWM, ADC, and USB peripherals.

## Hardware You Need

> **Required hardware**
> - Raspberry Pi **Pico 2W** (RP2350 + CYW43439) — not Pico 1, Pico W, or plain Pico 2
> - Data-capable USB-A → micro-USB cable (charge-only cables will not enumerate)
> - 2.4 GHz 802.11n Wi-Fi access point (5 GHz not supported by CYW43439)
> - Host machine with a USB port — no drivers needed for UF2 drag-and-drop

## Quick Start (End User)

### 1. Flash the firmware

Download the latest `pico-socketeer.uf2` from [Releases](../../releases).

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

See [PROTOCOL.md](PROTOCOL.md) for the full command reference.

## Developer Setup

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | ≥ 1.85 | `rustup toolchain install stable` |
| Embedded target | thumbv8m.main-none-eabihf | `rustup target add thumbv8m.main-none-eabihf` |
| flip-link | latest | `cargo install flip-link` |
| probe-rs | latest | `cargo install probe-rs-tools` |

### Build

```sh
# Clone and enter the repo
git clone https://github.com/<your-org>/pico-socketeer.git
cd pico-socketeer

# Download CYW43 firmware blobs (one-time setup)
mkdir cyw43-firmware
curl -fsSL https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware/43439A0.bin \
     -o cyw43-firmware/43439A0.bin
curl -fsSL https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware/43439A0_clm.bin \
     -o cyw43-firmware/43439A0_clm.bin

# Build firmware
cargo build --release --target thumbv8m.main-none-eabihf
```

### Flash (probe-rs)

Connect a debug probe (e.g. another Pico running picoprobe) and run:

```sh
cargo run --release --target thumbv8m.main-none-eabihf
```

The `.cargo/config.toml` runner is `probe-rs run --chip RP235x`.

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
├── net.rs                # CYW43 init, TCP server, LED driver (embedded-only)
├── lib.rs                # Library root (host + embedded)
├── protocol.rs           # JSON serialisation / deserialisation, framing
├── router.rs             # Command dispatcher (interface + action validation)
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

## Related Documents

- [PROTOCOL.md](PROTOCOL.md) — Full wire protocol specification
- [LED_STATUS.md](LED_STATUS.md) — LED blink pattern reference
- [CONTRIBUTING.md](CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](CHANGELOG.md) — Release history

## License

MIT — see [LICENSE](LICENSE).
