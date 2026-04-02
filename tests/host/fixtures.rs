use pico_conduit::protocol::Command;

/// Build a minimal [`Command`] with all fields `None` except `version`, `id`,
/// `interface`, and `action`. Shared across all host test modules so that
/// adding a new field to `Command` only requires updating this one place.
pub fn make_cmd<'a>(
    id: &'a str,
    interface: Option<&'a str>,
    action: Option<&'a str>,
) -> Command<'a> {
    Command {
        version: Some(1),
        id,
        interface,
        action,
        pin: None,
        value: None,
        mode: None,
        pull: None,
        uart: None,
        bytes: None,
        len: None,
        baud: None,
        data_bits: None,
        parity: None,
        stop_bits: None,
        spi: None,
        freq_hz: None,
        cpol: None,
        cpha: None,
        i2c: None,
        addr: None,
        write_bytes: None,
        read_len: None,
        channel: None,
        duty_u16: None,
        adc_channel: None,
        interval_ms: None,
        trigger: None,
        commands: None,
    }
}
