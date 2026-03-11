use pico_socketeer::board::{TCP_NODELAY, TCP_RX_BUF_SIZE, TCP_TX_BUF_SIZE};
use pico_socketeer::protocol::MAX_MSG_LEN;

#[test]
fn tcp_nodelay_is_enabled() {
    assert!(
        TCP_NODELAY,
        "TCP_NODELAY must be true to eliminate Nagle delay"
    );
}

#[test]
fn tcp_rx_buf_exceeds_max_msg_len() {
    assert!(
        TCP_RX_BUF_SIZE > MAX_MSG_LEN,
        "RX buffer must exceed MAX_MSG_LEN to allow headroom for TCP overhead"
    );
}

#[test]
fn tcp_tx_buf_exceeds_max_msg_len() {
    assert!(
        TCP_TX_BUF_SIZE > MAX_MSG_LEN,
        "TX buffer must exceed MAX_MSG_LEN to avoid stalls on large responses"
    );
}

#[test]
fn tcp_buf_sizes_are_power_of_two() {
    assert_eq!(
        TCP_RX_BUF_SIZE & (TCP_RX_BUF_SIZE - 1),
        0,
        "TCP_RX_BUF_SIZE should be a power of two for efficient ring-buffer arithmetic"
    );
    assert_eq!(
        TCP_TX_BUF_SIZE & (TCP_TX_BUF_SIZE - 1),
        0,
        "TCP_TX_BUF_SIZE should be a power of two for efficient ring-buffer arithmetic"
    );
}

#[cfg(feature = "pico2w")]
mod pico2w {
    use fixed::FixedU32;
    use fixed::types::extra::U8;
    use pico_socketeer::board::{
        CRED_FLASH_OFFSET, CRED_REGION_SIZE, CYW43_CLOCK_DIVIDER, FLASH_SIZE, validate_chip_part,
    };

    #[test]
    fn flash_size_is_4mb() {
        assert_eq!(FLASH_SIZE, 4 * 1024 * 1024);
    }

    #[test]
    fn cred_offset_is_last_8kb() {
        assert_eq!(CRED_FLASH_OFFSET, 0x3F_E000);
    }

    #[test]
    fn cred_offset_plus_region_equals_flash() {
        assert_eq!(CRED_FLASH_OFFSET as usize + CRED_REGION_SIZE, FLASH_SIZE);
    }

    #[test]
    fn validate_correct_chip_part() {
        assert!(validate_chip_part(0x4).is_ok());
    }

    #[test]
    fn validate_wrong_chip_part_rp2040() {
        assert!(validate_chip_part(0x2).is_err());
    }

    #[test]
    fn validate_wrong_chip_part_garbage() {
        assert!(validate_chip_part(0xFF).is_err());
    }

    #[test]
    fn cyc43_clock_divider_is_faster_than_default() {
        // DEFAULT_CLOCK_DIVIDER = 0x0200 (divider 2.0 → ~37.5 MHz SPI)
        // CYW43_CLOCK_DIVIDER   = 0x0180 (divider 1.5 → ~50 MHz SPI)
        // Smaller raw bits = smaller divisor = higher SPI clock
        let default = FixedU32::<U8>::from_bits(0x0200);
        assert!(
            CYW43_CLOCK_DIVIDER < default,
            "Pico 2W clock divider must be smaller than default to increase SPI speed"
        );
    }

    #[test]
    fn cyc43_clock_divider_does_not_exceed_chip_max() {
        // CYW43439 SPI max is 50 MHz; at 150 MHz sys clock, minimum safe divider is 1.5.
        // Raw bits 0x0180 == FixedU32<U8>(1.5). Anything smaller would exceed the chip rating.
        let min_safe = FixedU32::<U8>::from_bits(0x0180);
        assert!(
            CYW43_CLOCK_DIVIDER >= min_safe,
            "Clock divider too small; SPI clock would exceed CYW43439 50 MHz rating"
        );
    }
}

#[cfg(feature = "pico1w")]
mod pico1w {
    use fixed::FixedU32;
    use fixed::types::extra::U8;
    use pico_socketeer::board::{
        CRED_FLASH_OFFSET, CRED_REGION_SIZE, CYW43_CLOCK_DIVIDER, FLASH_SIZE, validate_chip_part,
    };

    #[test]
    fn flash_size_is_2mb() {
        assert_eq!(FLASH_SIZE, 2 * 1024 * 1024);
    }

    #[test]
    fn cred_offset_is_last_8kb() {
        assert_eq!(CRED_FLASH_OFFSET, 0x1F_E000);
    }

    #[test]
    fn cred_offset_plus_region_equals_flash() {
        assert_eq!(CRED_FLASH_OFFSET as usize + CRED_REGION_SIZE, FLASH_SIZE);
    }

    #[test]
    fn validate_correct_chip_part() {
        assert!(validate_chip_part(0x2).is_ok());
    }

    #[test]
    fn validate_wrong_chip_part_rp2350() {
        assert!(validate_chip_part(0x4).is_err());
    }

    #[test]
    fn validate_wrong_chip_part_garbage() {
        assert!(validate_chip_part(0xFF).is_err());
    }

    #[test]
    fn cyc43_clock_divider_is_faster_than_default() {
        // DEFAULT_CLOCK_DIVIDER = 0x0200 (divider 2.0 → ~31 MHz SPI)
        // CYW43_CLOCK_DIVIDER   = 0x0140 (divider 1.25 → ~50 MHz SPI)
        let default = FixedU32::<U8>::from_bits(0x0200);
        assert!(
            CYW43_CLOCK_DIVIDER < default,
            "Pico W clock divider must be smaller than default to increase SPI speed"
        );
    }

    #[test]
    fn cyc43_clock_divider_does_not_exceed_chip_max() {
        // CYW43439 SPI max is 50 MHz; at 125 MHz sys clock, minimum safe divider is 1.25.
        // Raw bits 0x0140 == FixedU32<U8>(1.25).
        let min_safe = FixedU32::<U8>::from_bits(0x0140);
        assert!(
            CYW43_CLOCK_DIVIDER >= min_safe,
            "Clock divider too small; SPI clock would exceed CYW43439 50 MHz rating"
        );
    }
}
