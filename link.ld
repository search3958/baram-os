OUTPUT_FORMAT(elf32-i386)
ENTRY(_start)
SECTIONS {
    . = 0x100000; /* 1MB地点から配置 */
    .text : { *(.multiboot) *(.text) }
    .data : { *(.data) }
    .bss  : { *(.bss) }
}