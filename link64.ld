OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

SECTIONS {
    . = 1M;

    .text : ALIGN(4096) {
        KEEP(*(.multiboot))
        *(.text)
        *(.rodata*)
    }

    .data : ALIGN(4096) {
        *(.data)
    }

    .bss : ALIGN(4096) {
        *(COMMON)
        *(.bss)
    }

    . = ALIGN(4096);
    _kernel_end = .;
}
