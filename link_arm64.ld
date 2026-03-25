ENTRY(_start)

SECTIONS
{
    . = 0x40000000;
    _kernel_start = .;

    .text : {
        KEEP(*(.text.boot))
        *(.text)
    }

    .rodata : {
        *(.rodata)
    }

    .data : {
        *(.data)
    }

    .bss : {
        _bss_start = .;
        *(.bss)
        *(COMMON)
        _bss_end = .;
    }

    . = ALIGN(16);
    _kernel_end = .;
}
