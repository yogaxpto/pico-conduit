//! Message router — dispatches parsed commands to the appropriate interface handler.
//!
//! The router validates `interface` and `action` fields and returns structured error responses
//! for unknown values. Each interface handler is responsible for further validation of its
//! own required parameters.

use crate::interfaces::{i2c, pwm, spi, uart, usb};
use crate::protocol::{
    AdcChannel, BatchResponse, Command, ERROR_ALREADY_SUBSCRIBED, ERROR_BATCH_EMPTY,
    ERROR_BATCH_TOO_LARGE, ERROR_MISSING_FIELD, ERROR_NOT_CONFIGURED, ERROR_NOT_SUBSCRIBED,
    ERROR_SUBSCRIPTION_LIMIT, ERROR_UNKNOWN_ACTION, ERROR_UNKNOWN_INTERFACE, EdgeTrigger,
    MAX_BATCH_SIZE, MAX_SUBSCRIPTIONS, Response, ResponseData, Subscription, SubscriptionTarget,
};
use core::fmt::Write as _;

/// Peripheral configuration state tracked across commands within a single TCP session.
///
/// Created at connection accept and dropped on disconnect. Configure actions update
/// the relevant fields; subsequent read/write actions check the `configured` flag.
pub struct DeviceState {
    pub uart: [uart::UartConfig; 2],
    pub spi: [spi::SpiConfig; 2],
    pub i2c: [i2c::I2cConfig; 2],
    pub usb_configured: bool,
    /// Wi-Fi SSID (populated by firmware, empty in host tests).
    pub config_ssid: heapless::String<32>,
    /// Device IP address (populated by firmware, empty in host tests).
    pub config_ip: heapless::String<16>,
    /// Whether the device is currently connected to Wi-Fi.
    pub config_connected: bool,
    /// MQTT broker host (populated by firmware when transport-mqtt is active).
    #[cfg(feature = "transport-mqtt")]
    pub config_mqtt_host: heapless::String<64>,
    /// MQTT broker port (populated by firmware when transport-mqtt is active).
    #[cfg(feature = "transport-mqtt")]
    pub config_mqtt_port: u16,
    /// Set by `system/reboot_to_bootloader`; checked by `net.rs` after the response is flushed.
    pub pending_reboot: bool,
    /// Active push subscriptions. Cleared on disconnect (`DeviceState` is dropped).
    pub subscriptions: heapless::Vec<Subscription, MAX_SUBSCRIPTIONS>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            uart: [uart::UartConfig::default(), uart::UartConfig::default()],
            spi: [spi::SpiConfig::default(), spi::SpiConfig::default()],
            i2c: [i2c::I2cConfig::default(), i2c::I2cConfig::default()],
            usb_configured: false,
            config_ssid: heapless::String::new(),
            config_ip: heapless::String::new(),
            config_connected: false,
            #[cfg(feature = "transport-mqtt")]
            config_mqtt_host: heapless::String::new(),
            #[cfg(feature = "transport-mqtt")]
            config_mqtt_port: 1883,
            pending_reboot: false,
            subscriptions: heapless::Vec::new(),
        }
    }
}

/// Routing table: each entry maps an interface name to its valid actions.
/// To add a new interface, add one row here — no other code in this file changes.
static VALID_ROUTES: &[(&str, &[&str])] = &[
    (
        "gpio",
        &["read", "write", "set_mode", "subscribe", "unsubscribe"],
    ),
    ("uart", &["read", "write", "configure"]),
    ("spi", &["transfer", "write", "configure"]),
    ("i2c", &["read", "write", "write_read", "configure"]),
    ("pwm", &["set_duty", "set_freq", "enable", "disable"]),
    ("adc", &["read", "subscribe", "unsubscribe"]),
    ("usb", &["read", "write"]),
    ("config", &["get"]),
    ("system", &["get_version", "reboot_to_bootloader"]),
    ("batch", &["run"]),
];

/// Dispatch a command to the appropriate interface handler.
///
/// This function matches on `cmd.interface` and delegates to the corresponding module.
/// Returns `Err` with an error code if the interface or action is unknown.
///
/// In the full firmware, each branch calls into `crate::interfaces::*::handle(cmd, hw)`.
/// In this routing layer we only validate the interface/action strings.
///
/// # Errors
///
/// Returns `Err` if the interface or action is absent or not in [`VALID_ROUTES`].
pub fn validate_route<'a>(cmd: &Command<'a>) -> Result<(&'a str, &'a str), Response<'a>> {
    let Some(interface) = cmd.interface else {
        return Err(Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE));
    };
    let Some(action) = cmd.action else {
        return Err(Response::error(cmd.id, ERROR_UNKNOWN_ACTION));
    };

    let actions = VALID_ROUTES
        .iter()
        .find(|(iface, _)| *iface == interface)
        .map(|(_, actions)| *actions)
        .ok_or_else(|| Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE))?;

    if !actions.contains(&action) {
        return Err(Response::error(cmd.id, ERROR_UNKNOWN_ACTION));
    }

    Ok((interface, action))
}

/// Dispatch a validated command to its interface handler, updating peripheral state as needed.
///
/// `route` is the `(interface, action)` tuple returned by [`validate_route`].
/// Actions that require hardware peripherals (GPIO pins, ADC reads, UART/SPI/I2C RX data)
/// return [`ERROR_NOT_CONFIGURED`] — the firmware extends this with actual peripheral access.
#[allow(clippy::too_many_lines, clippy::match_same_arms)]
pub fn dispatch<'a>(
    cmd: &Command<'a>,
    route: (&str, &str),
    state: &mut DeviceState,
) -> Response<'a> {
    // Apply peripheral configure: validate → store config → ok.
    macro_rules! configure {
        ($handler:path, $field:expr, $arr:expr) => {
            match $handler(cmd) {
                Ok(cfg) => {
                    $arr[$field.unwrap_or(0) as usize] = cfg;
                    Response::ok(cmd.id, None)
                }
                Err(r) => r,
            }
        };
    }

    // Dispatch a write-like action: validate index → call handler with configured flag.
    macro_rules! peripheral_write {
        ($validate:path, $arr:expr, $handler:path) => {{
            let idx = match $validate(cmd) {
                Ok(i) => i as usize,
                Err(r) => return r,
            };
            $handler(cmd, $arr[idx].configured)
        }};
    }

    match route {
        // ---- GPIO ----
        ("gpio", "set_mode") => crate::interfaces::gpio::handle_set_mode(cmd),
        ("gpio", "read" | "write") => {
            // Needs actual InputPin/OutputPin hardware — stub returns not_configured.
            Response::error(cmd.id, ERROR_NOT_CONFIGURED)
        }
        ("gpio", "subscribe") => {
            let Some(pin) = cmd.pin else {
                return Response::error(cmd.id, ERROR_MISSING_FIELD);
            };
            if let Some(trigger_str) = cmd.trigger {
                let Some(trigger) = EdgeTrigger::parse(trigger_str) else {
                    return Response::error(cmd.id, ERROR_MISSING_FIELD);
                };
                handle_subscribe(cmd.id, SubscriptionTarget::GpioEdge { pin, trigger }, state)
            } else {
                let interval_ms = cmd.interval_ms.unwrap_or(100);
                handle_subscribe(
                    cmd.id,
                    SubscriptionTarget::GpioLevel { pin, interval_ms },
                    state,
                )
            }
        }
        ("gpio", "unsubscribe") => {
            let Some(pin) = cmd.pin else {
                return Response::error(cmd.id, ERROR_MISSING_FIELD);
            };
            handle_unsubscribe_gpio(cmd.id, pin, state)
        }

        // ---- ADC ----
        ("adc", "read") => match crate::interfaces::adc::validate_read(cmd) {
            Ok(_channel) => Response::error(cmd.id, ERROR_NOT_CONFIGURED),
            Err(r) => r,
        },
        ("adc", "subscribe") => {
            let Some(channel) = cmd.adc_channel else {
                return Response::error(cmd.id, ERROR_MISSING_FIELD);
            };
            let interval_ms = cmd.interval_ms.unwrap_or(100);
            handle_subscribe(
                cmd.id,
                SubscriptionTarget::Adc {
                    channel,
                    interval_ms,
                },
                state,
            )
        }
        ("adc", "unsubscribe") => {
            let Some(channel) = cmd.adc_channel else {
                return Response::error(cmd.id, ERROR_MISSING_FIELD);
            };
            handle_unsubscribe_adc(cmd.id, channel, state)
        }

        // ---- UART ----
        ("uart", "configure") => configure!(uart::handle_configure, cmd.uart, state.uart),
        ("uart", "write") => peripheral_write!(uart::validate_uart, state.uart, uart::handle_write),
        ("uart", "read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- SPI ----
        ("spi", "configure") => configure!(spi::handle_configure, cmd.spi, state.spi),
        ("spi", "write") => peripheral_write!(spi::validate_spi, state.spi, spi::handle_write),
        ("spi", "transfer") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- I2C ----
        ("i2c", "configure") => configure!(i2c::handle_configure, cmd.i2c, state.i2c),
        ("i2c", "write") => peripheral_write!(i2c::validate_i2c, state.i2c, i2c::handle_write),
        ("i2c", "read" | "write_read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- PWM ----
        ("pwm", "set_duty") => pwm::handle_set_duty(cmd),
        ("pwm", "set_freq") => pwm::handle_set_freq(cmd),
        ("pwm", "enable") => pwm::handle_enable(cmd),
        ("pwm", "disable") => pwm::handle_disable(cmd),

        // ---- USB ----
        ("usb", "write") => usb::handle_write(cmd, state.usb_configured),
        ("usb", "read") => Response::error(cmd.id, ERROR_NOT_CONFIGURED),

        // ---- Config ----
        ("config", "get") => Response::ok(
            cmd.id,
            Some(ResponseData::Config {
                ssid: state.config_ssid.clone(),
                ip: state.config_ip.clone(),
                connected: state.config_connected,
                #[cfg(feature = "transport-mqtt")]
                mqtt_host: state.config_mqtt_host.clone(),
                #[cfg(feature = "transport-mqtt")]
                mqtt_port: state.config_mqtt_port,
            }),
        ),

        // ---- System ----
        ("system", "get_version") => {
            let mut version: heapless::String<16> = heapless::String::new();
            let _ = version.write_str(env!("CARGO_PKG_VERSION"));
            Response::ok(cmd.id, Some(ResponseData::Version { version }))
        }
        ("system", "reboot_to_bootloader") => {
            state.pending_reboot = true;
            Response::ok(cmd.id, None)
        }

        // ---- Batch ----
        // Batch is handled by dispatch_batch(); reaching here means the caller
        // used dispatch() directly on a batch command, which is not supported.
        ("batch", "run") => Response::error(cmd.id, ERROR_UNKNOWN_INTERFACE),

        // validate_route already rejected invalid routes
        _ => unreachable!(),
    }
}

/// Dispatch a `batch/run` command, executing each inner command in order.
///
/// Returns a [`BatchResponse`] containing one [`Response`] per inner command.
/// Errors at the batch level (empty list, too many commands) set `ok = false` and
/// return a single error code — no inner responses are included.
///
/// Each inner command is dispatched independently; errors in one command do not
/// abort subsequent commands.
pub fn dispatch_batch<'a>(cmd: &Command<'a>, state: &mut DeviceState) -> BatchResponse<'a> {
    let inner_cmds = match cmd.commands.as_ref() {
        None => {
            return BatchResponse {
                id: cmd.id,
                ok: false,
                responses: heapless::Vec::new(),
                error: Some(ERROR_BATCH_EMPTY),
            };
        }
        Some(v) if v.is_empty() => {
            return BatchResponse {
                id: cmd.id,
                ok: false,
                responses: heapless::Vec::new(),
                error: Some(ERROR_BATCH_EMPTY),
            };
        }
        Some(v) => v,
    };

    if inner_cmds.len() > MAX_BATCH_SIZE {
        return BatchResponse {
            id: cmd.id,
            ok: false,
            responses: heapless::Vec::new(),
            error: Some(ERROR_BATCH_TOO_LARGE),
        };
    }

    let mut responses: heapless::Vec<Response<'a>, MAX_BATCH_SIZE> = heapless::Vec::new();
    #[allow(clippy::explicit_iter_loop)] // heapless::Vec doesn't impl IntoIterator for &
    for inner in inner_cmds.iter() {
        let full_cmd = inner.to_command();
        let resp = match validate_route(&full_cmd) {
            Err(r) => r,
            Ok(route) => dispatch(&full_cmd, route, state),
        };
        // Vec capacity equals MAX_BATCH_SIZE and we checked len above, so push never fails.
        let _ = responses.push(resp);
    }

    BatchResponse {
        id: cmd.id,
        ok: true,
        responses,
        error: None,
    }
}

// ── Subscription helpers ──────────────────────────────────────────────────────

/// Register a new push subscription. Returns error if the limit is exceeded or a duplicate exists.
fn handle_subscribe<'a>(
    id: &'a str,
    target: SubscriptionTarget,
    state: &mut DeviceState,
) -> Response<'a> {
    // Reject duplicates (same target regardless of id or interval).
    if state.subscriptions.iter().any(|s| s.same_target(&target)) {
        return Response::error(id, ERROR_ALREADY_SUBSCRIBED);
    }
    if state.subscriptions.len() >= MAX_SUBSCRIPTIONS {
        return Response::error(id, ERROR_SUBSCRIPTION_LIMIT);
    }
    let mut sub_id: heapless::String<32> = heapless::String::new();
    let _ = sub_id.push_str(id);
    let sub = Subscription { id: sub_id, target };
    let _ = state.subscriptions.push(sub);
    Response::ok(id, None)
}

/// Remove an ADC subscription by channel. Returns `not_subscribed` if none found.
fn handle_unsubscribe_adc<'a>(
    id: &'a str,
    channel: AdcChannel,
    state: &mut DeviceState,
) -> Response<'a> {
    let before = state.subscriptions.len();
    state.subscriptions.retain(|s| {
        if let SubscriptionTarget::Adc { channel: c, .. } = &s.target {
            *c != channel
        } else {
            true
        }
    });
    if state.subscriptions.len() == before {
        return Response::error(id, ERROR_NOT_SUBSCRIBED);
    }
    Response::ok(id, None)
}

/// Remove a GPIO subscription (level or edge) by pin. Returns `not_subscribed` if none found.
fn handle_unsubscribe_gpio<'a>(id: &'a str, pin: u8, state: &mut DeviceState) -> Response<'a> {
    let before = state.subscriptions.len();
    state.subscriptions.retain(|s| match &s.target {
        SubscriptionTarget::GpioLevel { pin: p, .. }
        | SubscriptionTarget::GpioEdge { pin: p, .. } => *p != pin,
        SubscriptionTarget::Adc { .. } => true,
    });
    if state.subscriptions.len() == before {
        return Response::error(id, ERROR_NOT_SUBSCRIBED);
    }
    Response::ok(id, None)
}
