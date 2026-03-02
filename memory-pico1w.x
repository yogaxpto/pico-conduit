MEMORY {
    BOOT2       : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH       : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100 - 8K
    CREDENTIALS : ORIGIN = 0x101FE000, LENGTH = 8K
    RAM         : ORIGIN = 0x20000000, LENGTH = 264K
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
