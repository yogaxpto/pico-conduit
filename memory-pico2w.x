/* BOOT2 reserves 0x200 (not 0x100) so that FLASH — and therefore cortex-m-rt's
   .vector_table, which is placed at ORIGIN(FLASH) — is 512-byte aligned.
   The RP2350 vector table is 16 core entries + 53 device IRQs = 276 bytes, and
   Armv8-M requires the table to be aligned to the next power of two >= its size
   (cortex-m-rt >= 0.7.6 asserts this at link time). The RP2350 has no RP2040-style
   256-byte second-stage bootloader, so the extra 256 bytes here are unused padding;
   the bootrom locates the image via the IMAGE_DEF block in the first 4 KB. */
MEMORY {
    BOOT2       : ORIGIN = 0x10000000, LENGTH = 0x200
    FLASH       : ORIGIN = 0x10000200, LENGTH = 4096K - 0x200 - 8K
    CREDENTIALS : ORIGIN = 0x103FE000, LENGTH = 8K
    RAM         : ORIGIN = 0x20000000, LENGTH = 520K
}

SECTIONS {
    /* ### Boot loader */
    .boot2 ORIGIN(BOOT2) :
    {
        KEEP(*(.boot2));
    } > BOOT2
} INSERT BEFORE .text;

SECTIONS {
    /* ### Boot ROM info — picotool can find it in the first 512 bytes of flash */
    .boot_info : ALIGN(4)
    {
        KEEP(*(.boot_info));
    } > FLASH
} INSERT AFTER .vector_table;

/* move .text to start after the boot info */
_stext = ADDR(.boot_info) + SIZEOF(.boot_info);

SECTIONS {
    /* ### Picotool Binary Info entries */
    .bi_entries : ALIGN(4)
    {
        __bi_entries_start = .;
        KEEP(*(.bi_entries));
        . = ALIGN(4);
        __bi_entries_end = .;
    } > FLASH
} INSERT AFTER .text;

SECTIONS {
    .flash_end : {
        __flash_binary_end = .;
    } > FLASH
} INSERT AFTER .uninit;

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
