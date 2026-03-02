#[cfg(feature = "pico2w")]
mod pico2w {
    use pico_socketeer::board::{
        CRED_FLASH_OFFSET, CRED_REGION_SIZE, FLASH_SIZE, validate_chip_part,
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
}

#[cfg(feature = "pico1w")]
mod pico1w {
    use pico_socketeer::board::{
        CRED_FLASH_OFFSET, CRED_REGION_SIZE, FLASH_SIZE, validate_chip_part,
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
}
