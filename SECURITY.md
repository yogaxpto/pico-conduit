# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | Yes                |
| < 0.1   | No                 |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use GitHub Security Advisories to report vulnerabilities privately:

<https://github.com/yogaxpto/pico-conduit/security/advisories/new>

When reporting, please include:

- A description of the vulnerability and its potential impact
- Steps to reproduce the issue
- Any relevant logs, configuration, or firmware version information
- Your assessment of the severity (if possible)

## Response Timeline

- **72 hours** — We will acknowledge receipt of your report.
- **14 days** — We will provide a status update with our assessment and planned remediation.
- **Ongoing** — We will notify you when the fix is released.

If the vulnerability is accepted, we will coordinate disclosure with you and credit you
in the release notes (unless you prefer to remain anonymous).

If the vulnerability is declined (e.g., out of scope), we will explain our reasoning.

## Security Considerations

pico-conduit is firmware for a network-exposed microcontroller. The following aspects
are particularly relevant to its security posture:

- **No authentication by default.** Any client that can reach the device over the network
  can issue commands. Deployments should rely on network-level controls (firewalls,
  VLANs, isolated Wi-Fi networks) to restrict access.
- **Single-connection constraint.** The server accepts exactly one TCP/WebSocket/MQTT
  client at a time. This limits the attack surface by preventing concurrent exploitation
  but does not substitute for authentication.
- **Physical impact.** Commands received over the protocol directly control GPIO, PWM,
  SPI, and I2C peripherals. A malicious client can actuate real hardware, which may have
  safety implications depending on what is connected to the board.
- **Wi-Fi credentials in flash.** Wi-Fi SSID and password are stored in the CREDENTIALS
  region of flash memory. An attacker with firmware read access or physical access to the
  board could extract these credentials.
- **No TLS support.** All network traffic is currently unencrypted. Commands and responses
  (including any sensitive data) are transmitted in plaintext.

## Scope

### In-scope

The following categories of issues are considered valid security vulnerabilities:

- Firmware vulnerabilities that allow unauthorized command execution
- Protocol injection or parsing flaws that bypass validation
- Credential storage issues (e.g., credentials leaking through unintended channels)
- Memory safety issues in the firmware (buffer overflows, out-of-bounds access)
- Vulnerabilities in the Wi-Fi provisioning flow

### Out-of-scope

The following are **not** considered vulnerabilities for the purposes of this policy:

- **Physical access attacks** — extracting flash contents, JTAG/SWD debugging, or other
  attacks that require physical access to the board
- **Denial of service against the single connection** — the single-connection design is
  intentional and documented; occupying the sole connection slot is expected behavior
- **Issues in third-party dependencies** not maintained by this project — please report
  these to the upstream maintainers directly
- **Lack of TLS or authentication** — these are known limitations documented above, not
  vulnerabilities to report (contributions to add them are welcome)
