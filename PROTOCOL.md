# pico-socketeer Wire Protocol

## Overview

pico-socketeer exposes a newline-delimited JSON protocol over a TCP socket (port **4242**).
Each request is a single JSON object terminated by `\n`. The server returns a single JSON
response object terminated by `\n`. One connection handles one in-flight request at a time.

## Framing

- **Encoding:** UTF-8 JSON
- **Delimiter:** `\n` (0x0A) — the newline terminates each message
- **Maximum message size:** 512 bytes including the trailing newline
- **Single-connection:** the server accepts exactly one client at a time; the connection
  closes before a new `accept()` is issued
- **Idle timeout:** the server closes an idle connection after **30 seconds** of inactivity

## Request Format

```json
{
  "version": 1,
  "id": "<opaque string>",
  "interface": "<gpio|uart|spi|i2c|pwm|adc|usb|config>",
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
| `set_mode` | `pin: u8`, `mode: "input"\|"output"\|"input_pullup"\|"input_pulldown"` | _(none)_ |

**Pin restrictions:** GPIO29 is reserved (CYW43 SPI DIO); use returns `invalid_pin`.

---

### `uart`

| Action      | Additional fields | Response `data` |
|-------------|-------------------|-----------------|
| `configure` | `uart: 0\|1`, `baud: u32`, `data_bits: 5-9`, `parity: "none"\|"even"\|"odd"`, `stop_bits: 1\|2` | _(none)_ |
| `write`     | `uart: 0\|1`, `bytes: [u8, ...]` | _(none)_ |
| `read`      | `uart: 0\|1`, `len: usize` | `{"bytes": [...]}` |

---

### `spi`

| Action      | Additional fields | Response `data` |
|-------------|-------------------|-----------------|
| `configure` | `spi: 0\|1`, `freq_hz: u32`, `cpol: 0\|1`, `cpha: 0\|1` | _(none)_ |
| `write`     | `spi: 0\|1`, `bytes: [u8, ...]` | _(none)_ |
| `transfer`  | `spi: 0\|1`, `bytes: [u8, ...]` | `{"bytes": [...]}` |

---

### `i2c`

| Action       | Additional fields | Response `data` |
|--------------|-------------------|-----------------|
| `configure`  | `i2c: 0\|1`, `freq_hz: u32` | _(none)_ |
| `write`      | `i2c: 0\|1`, `addr: u8`, `bytes: [u8, ...]` | _(none)_ |
| `read`       | `i2c: 0\|1`, `addr: u8`, `len: usize` | `{"bytes": [...]}` |
| `write_read` | `i2c: 0\|1`, `addr: u8`, `write_bytes: [u8, ...]`, `read_len: usize` | `{"bytes": [...]}` |

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
| `write` | `bytes: [u8, ...]` | _(none)_ |
| `read`  | `len: usize` | `{"bytes": [...]}` |

---

### `config`

| Action | Additional fields | Response `data` |
|--------|-------------------|-----------------|
| `get`  | _(none)_          | `{"ssid": "...", "ip": "...", "connected": bool}` |

---

## Error Codes

All error codes are lowercase snake_case `&str` values in the `"error"` field.

| Code | Trigger |
|------|---------|
| `missing_version` | `version` field absent |
| `unsupported_version` | `version != 1` |
| `msg_too_large` | Frame exceeds 512 bytes (including newline) |
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
