# pico-socketeer Wire Protocol

## Overview

pico-socketeer exposes a JSON command/response protocol over one of three compile-time
selectable transports: **TCP** (default), **WebSocket**, or **MQTT**. The JSON schema,
interface handlers, and error codes are identical across all transports — only the framing
and connection management differ.

## Framing

- **Encoding:** UTF-8 JSON
- **Delimiter:** `\n` (0x0A) — the newline terminates each message
- **Maximum message size:** 1024 bytes including the trailing newline
- **Byte encoding:** All byte payloads (`bytes`, `write_bytes`) use standard base64 strings
  (alphabet A–Z, a–z, 0–9, +, /; `=` padding). Maximum decoded payload: 512 bytes
- **Single-connection:** the server accepts exactly one client at a time; the connection
  closes before a new `accept()` is issued
- **Idle timeout:** the server closes an idle connection after **30 seconds** of inactivity

## Pipelining

Clients **MAY** send multiple newline-delimited commands without waiting for each response.
The server processes commands sequentially from the TCP stream and emits responses in the
same order as commands were received.

**Ordering guarantee:** responses are always emitted in the same order commands arrive,
regardless of how they are buffered at the TCP layer.

**Error tolerance:** a malformed or invalid command returns an error response for that
command only. Subsequent commands in the pipeline continue to be processed normally —
the connection is never terminated by a protocol-level error.

Pipelining works best combined with `TCP_NODELAY` (enabled by default), which flushes
each response immediately without Nagle coalescing.

**Example — 5 commands sent in one burst:**
```sh
printf '%s\n' \
  '{"version":1,"id":"1","interface":"gpio","action":"set_mode","pin":0,"mode":"output"}' \
  '{"version":1,"id":"2","interface":"gpio","action":"write","pin":0,"value":1}' \
  '{"version":1,"id":"3","interface":"gpio","action":"write","pin":0,"value":0}' \
  '{"version":1,"id":"4","interface":"adc","action":"read","adc_channel":0}' \
  '{"version":1,"id":"5","interface":"system","action":"get_version"}' \
  | nc <pico-ip> 4242
# All five responses arrive in order: id 1 through 5.
```

## Request Format

```json
{
  "version": 1,
  "id": "<opaque string>",
  "interface": "<gpio|uart|spi|i2c|pwm|adc|usb|config|system|batch>",
  "action": "<action>",
  ...interface-specific fields...
}
```

| Field       | Type    | Required | Description |
|-------------|---------|----------|-------------|
| `version`   | integer | Yes      | Protocol version — must be `1` |
| `id`        | string  | Yes      | Client-chosen request ID; echoed verbatim in the response |
| `interface` | string  | Yes      | Target peripheral (see below) |
| `action`    | string  | Yes      | Operation to perform (see below) |

## Response Format

```json
{
  "id": "<echoed from request>",
  "ok": true,
  "data": { ...interface-specific fields... }
}
```

On error:
```json
{
  "id": "<echoed from request, or empty string>",
  "ok": false,
  "error": "<error_code>"
}
```

---

## Interfaces and Actions

### `gpio`

| Action     | Additional fields            | Response `data` |
|------------|------------------------------|-----------------|
| `read`     | `pin: u8`                    | `{"value": 0\|1}` |
| `write`    | `pin: u8`, `value: 0\|1`    | _(none)_ |
| `set_mode` | `pin: u8`, `mode: "input"\|"output"`, `pull?: "up"\|"down"\|"none"` | _(none)_ |

**`set_mode` pull configuration:** The optional `pull` field configures the internal pull
resistor. When omitted it defaults to `"none"`.

**Pin restrictions:** GPIO29 is reserved (CYW43 SPI DIO); use returns `invalid_pin`.

---

### `uart`

| Action      | Additional fields | Response `data` |
|-------------|-------------------|-----------------|
| `configure` | `uart: 0\|1`, `baud: u32`, `data_bits: 5-9`, `parity: "none"\|"even"\|"odd"`, `stop_bits: 1\|2` | _(none)_ |
| `write`     | `uart: 0\|1`, `bytes: "<base64>"` | _(none)_ |
| `read`      | `uart: 0\|1`, `len: usize` | `{"bytes": "<base64>"}` |

---

### `spi`

| Action      | Additional fields | Response `data` |
|-------------|-------------------|-----------------|
| `configure` | `spi: 0\|1`, `freq_hz: u32`, `cpol: 0\|1`, `cpha: 0\|1` | _(none)_ |
| `write`     | `spi: 0\|1`, `bytes: "<base64>"` | _(none)_ |
| `transfer`  | `spi: 0\|1`, `bytes: "<base64>"` | `{"bytes": "<base64>"}` |

---

### `i2c`

| Action       | Additional fields | Response `data` |
|--------------|-------------------|-----------------|
| `configure`  | `i2c: 0\|1`, `freq_hz: u32` | _(none)_ |
| `write`      | `i2c: 0\|1`, `addr: u8`, `bytes: "<base64>"` | _(none)_ |
| `read`       | `i2c: 0\|1`, `addr: u8`, `len: usize` | `{"bytes": "<base64>"}` |
| `write_read` | `i2c: 0\|1`, `addr: u8`, `write_bytes: "<base64>"`, `read_len: usize` | `{"bytes": "<base64>"}` |

---

### `pwm`

| Action      | Additional fields | Response `data` |
|-------------|-------------------|-----------------|
| `set_duty`  | `channel: 0-7`, `duty_u16: u16` | _(none)_ |
| `set_freq`  | `channel: 0-7`, `freq_hz: u32` | _(none)_ |
| `enable`    | `channel: 0-7` | _(none)_ |
| `disable`   | `channel: 0-7` | _(none)_ |

---

### `adc`

| Action | Additional fields | Response `data` |
|--------|-------------------|-----------------|
| `read` | `adc_channel: 0\|1\|2\|3` | `{"raw": u16, "voltage": f32}` for channels 0-2; `{"celsius": f32}` for channel 3 (onboard temperature) |

Channel mapping: `0` = GPIO26, `1` = GPIO27, `2` = GPIO28, `3` = onboard temperature sensor.

---

### `usb`

| Action  | Additional fields | Response `data` |
|---------|-------------------|-----------------|
| `write` | `bytes: "<base64>"` | _(none)_ |
| `read`  | `len: usize` | `{"bytes": "<base64>"}` |

---

### `config`

| Action | Additional fields | Response `data` |
|--------|-------------------|-----------------|
| `get`  | _(none)_          | `{"ssid": "...", "ip": "...", "connected": bool}` |

### `batch`

Executes multiple commands in a single round-trip, reducing Wi-Fi latency overhead.

| Action | Additional fields | Response `data` |
|--------|-------------------|-----------------|
| `run`  | `commands: [{...}, ...]` | `{"responses": [{...}, ...]}` |

**Request:**
```json
{"version":1,"id":"b1","interface":"batch","action":"run","commands":[
  {"version":1,"id":"1","interface":"gpio","action":"read","pin":0},
  {"version":1,"id":"2","interface":"adc","action":"read","adc_channel":0},
  {"version":1,"id":"3","interface":"gpio","action":"write","pin":1,"value":1}
]}
```

**Response:**
```json
{"id":"b1","ok":true,"data":{"responses":[
  {"id":"1","ok":false,"data":null,"error":"not_configured"},
  {"id":"2","ok":false,"data":null,"error":"not_configured"},
  {"id":"3","ok":false,"data":null,"error":"not_configured"}
]},"error":null}
```

**Constraints:**
- Maximum batch size: **16 commands**. Larger batches return `batch_too_large`.
- Empty `commands` array returns `batch_empty`.
- Each inner command is processed independently; an error in one command does not abort subsequent commands.
- The serialized response must fit within `MAX_MSG_LEN` (1024 bytes); if it overflows, `msg_too_large` is returned for the whole batch.
- Batches cannot be nested (no `batch/run` inside `commands`).

---

### `system`

| Action | Additional fields | Response `data` |
|--------|-------------------|-----------------|
| `get_version` | _(none)_ | `{"version": "0.1.0"}` (firmware version from `Cargo.toml`) |
| `reboot_to_bootloader` | _(none)_ | _(none)_ |

#### `system/get_version`

Returns the firmware version string.

```json
{"version":1,"id":"1","interface":"system","action":"get_version"}
```
Response:
```json
{"id":"1","ok":true,"data":{"version":"0.1.0"},"error":null}
```

#### `system/reboot_to_bootloader`

Instructs the device to reboot into USB bootloader mode. The device sends the `ok`
response, flashes the LED 10 times rapidly, then calls the RP2350 ROM to enter USB
bootloader mode. The TCP connection will close.

After rebooting, the Pico 2W presents as a USB mass storage drive named **RPI-RP2**.
Drag a new `.uf2` firmware file onto the drive to update the firmware. The device reboots
automatically when the copy completes.

> **Note:** A physical USB cable connecting the Pico 2W to a computer is required to
> complete the update after the device reboots into bootloader mode.

```json
{"version":1,"id":"1","interface":"system","action":"reboot_to_bootloader"}
```
Response:
```json
{"id":"1","ok":true,"data":null,"error":null}
```
*(TCP connection closes; device reboots into USB bootloader)*

---

## Error Codes

All error codes are lowercase snake_case `&str` values in the `"error"` field.

| Code | Trigger |
|------|---------|
| `missing_version` | `version` field absent |
| `unsupported_version` | `version != 1` |
| `msg_too_large` | Frame exceeds 1024 bytes (including newline) |
| `malformed_json` | JSON parse failure |
| `missing_field` | Required field for the action is absent |
| `unknown_interface` | `interface` value not recognised |
| `unknown_action` | `action` value not recognised for the interface |
| `invalid_pin` | Pin number out of range or reserved (e.g. GPIO29) |
| `value_out_of_range` | Numeric argument exceeds allowed range |
| `pin_in_use` | Pin already claimed by another peripheral |
| `not_configured` | Peripheral used before `configure` action |
| `peripheral_busy` | Peripheral busy with another operation |
| `peripheral_error` | Hardware-level error during operation |
| `invalid_encoding` | Base64 `bytes` or `write_bytes` field is malformed |
| `ws_handshake_failed` | WebSocket HTTP upgrade handshake failed (missing key, bad headers) |
| `batch_empty` | `batch/run` received an empty `commands` array |
| `batch_too_large` | `batch/run` received more than 16 commands |
| `already_subscribed` | `subscribe` for a channel/pin that already has an active subscription |
| `not_subscribed` | `unsubscribe` for a channel/pin with no active subscription |
| `subscription_limit` | Subscribe would exceed the maximum 8 concurrent subscriptions |

---

## Binary Codec (optional)

By default pico-socketeer uses UTF-8 JSON for all commands and responses. An optional
**postcard** binary codec can be selected at compile time via the `codec-postcard` feature flag.

### Codec evaluation

| Crate | Format | `no_std` + no-alloc | serde-compatible | Chosen |
|-------|--------|---------------------|------------------|--------|
| `rmp-serde` | MessagePack | ✗ (requires alloc) | ✓ | ✗ |
| `minicbor` | CBOR | ✓ | ✗ (own derive macros) | ✗ |
| `postcard` | postcard binary | ✓ | ✓ | **✓** |

### Size comparison

| Command / Response | JSON | Postcard | Reduction |
|-------------------|------|----------|-----------|
| GPIO read command | ~80 bytes | ~15 bytes | ~5× |
| GPIO read response | ~45 bytes | ~8 bytes | ~5× |
| Error response | ~55 bytes | ~12 bytes | ~4× |

### Building with the binary codec

```sh
# Pico 2W — TCP transport + postcard codec
cargo build --release --no-default-features --features embedded,pico2w,transport-tcp,codec-postcard

# Host tests with postcard codec
cargo test --test host --no-default-features --features codec-postcard --target aarch64-unknown-linux-musl
```

### Wire format (postcard)

Commands from client to device use postcard's compact binary encoding in place of JSON.
Responses use a flat tagged-enum binary format (see `src/codec.rs` — `BinaryResponse`).
There is no newline delimiter; the transport framing (TCP length, WebSocket frame) carries
the message boundary.

**Encoding rules:**
- All integer fields: postcard variable-length integer (varint)
- Optional fields: `0x00` = absent, `0x01` followed by value = present
- Strings (`id`, `interface`, `action`, etc.): varint length + UTF-8 bytes
- Enum variants: 1-byte discriminant followed by variant payload
- Byte payloads: varint length + raw bytes (no base64 encoding needed)

### Runtime negotiation

There is no runtime codec negotiation. The codec is selected at firmware compile time.
Client software must use the matching codec for the connected device. Mixing a JSON
client with a postcard firmware (or vice versa) will result in parse errors.

### Mutual exclusion

Codec features are mutually exclusive. Only one binary codec may be active at a time.
A `compile_error!` is emitted if two codec features are enabled simultaneously.

---

## Transport: TCP (default)

- **Port:** 4242
- **Framing:** newline-delimited JSON (`\n` terminates each message)
- **Connection model:** the server accepts exactly one client at a time
- **Idle timeout:** 30 seconds of inactivity closes the connection

Build with: `cargo build --release` (default features include `transport-tcp`)

---

## Transport: WebSocket

- **Port:** 4243
- **Framing:** WebSocket text frames (opcode 0x1); one JSON command per frame
- **Connection model:** the server accepts exactly one client at a time
- **Handshake:** standard HTTP/1.1 upgrade (RFC 6455)
- **Masking:** client-to-server frames must be masked; server-to-client frames are unmasked
- **Max payload:** 1024 bytes per frame
- **Fragmentation:** not supported (single-frame messages only)
- **Ping/Pong:** handled transparently by the server
- **TLS:** not supported

Build with:
```sh
cargo build --release --no-default-features --features embedded,pico2w,transport-websocket
```

### WebSocket example

```sh
websocat ws://<pico-ip>:4243
{"version":1,"id":"1","interface":"gpio","action":"read","pin":0}
# → {"id":"1","ok":true,"data":{"value":0},"error":null}
```

---

## Transport: MQTT

- **Broker:** external MQTT broker (configured via provisioning portal)
- **Port:** broker-defined (default 1883)
- **Protocol:** MQTT 5.0 (QoS 0, compatible with 3.1.1 brokers)
- **Topics:**
  - Command (client → device): `pico/<last4hex>/cmd`
  - Response (device → client): `pico/<last4hex>/resp`
  - `<last4hex>` is the lowercase hex of the last two bytes of the device's MAC address
- **Client ID:** `pico-<last4hex>`
- **Framing:** each MQTT PUBLISH payload is a complete JSON command or response (no newline delimiter needed)
- **Connection model:** single-command-at-a-time; device processes one command per PUBLISH, publishes response, then waits for next
- **Reconnect backoff:** 5s → 10s → 20s → 40s → 60s cap; resets on successful connection
- **Device state:** `DeviceState` (configured peripherals) resets on broker reconnect

Build with:
```sh
cargo build --release --no-default-features --features embedded,pico2w,transport-mqtt
```

### MQTT `config/get` response

When built with `transport-mqtt`, the `config/get` response includes additional fields:

```json
{"id":"1","ok":true,"data":{"ssid":"MyNet","ip":"192.168.1.42","connected":true,"mqtt_host":"192.168.1.100","mqtt_port":1883},"error":null}
```

### MQTT example (using mosquitto_pub/sub)

```sh
# Subscribe to responses
mosquitto_sub -h <broker> -t 'pico/a3f2/resp'

# Send a command
mosquitto_pub -h <broker> -t 'pico/a3f2/cmd' -m '{"version":1,"id":"1","interface":"gpio","action":"read","pin":0}'
# → {"id":"1","ok":true,"data":{"value":0},"error":null}
```

---

## Examples

**GPIO write:**
```json
{"version":1,"id":"req-1","interface":"gpio","action":"write","pin":15,"value":1}
```
Response:
```json
{"id":"req-1","ok":true}
```

**ADC read (channel 0):**
```json
{"version":1,"id":"req-2","interface":"adc","action":"read","adc_channel":0}
```
Response:
```json
{"id":"req-2","ok":true,"data":{"raw":2048,"voltage":1.65}}
```

**ADC temperature read:**
```json
{"version":1,"id":"req-3","interface":"adc","action":"read","adc_channel":3}
```
Response:
```json
{"id":"req-3","ok":true,"data":{"celsius":27.4}}
```

**Error (missing version):**
```json
{"id":"req-4","interface":"gpio","action":"read","pin":10}
```
Response:
```json
{"id":"","ok":false,"error":"missing_version"}
```
