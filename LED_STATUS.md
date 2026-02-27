# pico-socketeer LED Status Reference

The onboard LED (driven via CYW43 GPIO0) communicates firmware state through blink patterns.

## State Table

| State | Pattern | Description |
|-------|---------|-------------|
| `Booting` | 3 quick flashes, 1 s pause (repeats) | Firmware initialising, CYW43 starting up |
| `Provisioning` | 1 s on, 1 s off (slow blink) | No credentials — waiting for Wi-Fi setup via captive portal |
| `Scanning` | 2 quick flashes, 700 ms pause (repeats) | Scanning for Wi-Fi networks |
| `Connecting` | 100 ms on, 100 ms off (fast blink) | Attempting to join the configured Wi-Fi network |
| `Connected` | Solid ON | Joined Wi-Fi, DHCP obtained, TCP server listening on port 4242 |
| `Reconnecting` | 250 ms on, 250 ms off (medium blink) | Lost Wi-Fi — reconnect backoff in progress |
| `Error` | SOS Morse pattern (see below) | 10 minutes of failed reconnections; requires power cycle or fix |
| `Saving` | 5 rapid flashes | Writing credentials to flash |

## Timing Diagrams

Times are in milliseconds. `+` = LED on, `-` = LED off.

### Booting
```
+100-100+100-100+100-100|____1000____|+100-100+100-100+100-100|___...
```

### Provisioning
```
+___1000___|-___1000___|+___1000___|-___...
```

### Scanning
```
+100-100+100-100|_700_|+100-100+100-100|___...
```

### Connecting
```
+100-100+100-100+100-100+100-100+100-...
```

### Connected
```
+++++++++++++++++++++++ (solid on until state changes)
```

### Reconnecting
```
+250-250+250-250+250-250+250-...
```

### Error (SOS Morse)
```
S (3 dits):  +100-100+100-100+100-300
O (3 dahs):  +300-100+300-100+300-300
S (3 dits):  +100-100+100-100+100-2000
             (2 s inter-message gap, then repeats)
```

Full SOS timing sequence (ms on/off pairs):

| # | LED | Duration (ms) | Note |
|---|-----|---------------|------|
| 1 | ON  | 100 | S dit 1 |
| 2 | OFF | 100 | |
| 3 | ON  | 100 | S dit 2 |
| 4 | OFF | 100 | |
| 5 | ON  | 100 | S dit 3 |
| 6 | OFF | 300 | letter gap |
| 7 | ON  | 300 | O dah 1 |
| 8 | OFF | 100 | |
| 9 | ON  | 300 | O dah 2 |
| 10 | OFF | 100 | |
| 11 | ON  | 300 | O dah 3 |
| 12 | OFF | 300 | letter gap |
| 13 | ON  | 100 | S dit 1 |
| 14 | OFF | 100 | |
| 15 | ON  | 100 | S dit 2 |
| 16 | OFF | 100 | |
| 17 | ON  | 100 | S dit 3 |
| 18 | OFF | 2000 | inter-message gap |

### Saving
```
+100-100+100-100+100-100+100-100+100-100 (5 flashes then off)
```

## Notes

- The LED is driven through the CYW43 chip (`gpio_set(0, …)`) — it is **not** a standard
  RP2350 GPIO pin and therefore cannot be read or controlled by the JSON protocol.
- State transitions are immediate: when a new `LedState` is signalled, the current pattern
  exits at the next natural pause point.
- In `Error` state, the SOS pattern loops indefinitely. Recovery requires a power cycle or
  fixing the underlying Wi-Fi issue and rebooting.
