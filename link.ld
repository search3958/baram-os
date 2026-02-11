OUTPUT_FORMAT(elf32-i386)
ENTRY(_start)

SECTIONS {
    /* 1MBから配置を開始するが、.multibootを先頭に持ってくる */
    . = 1M;

    .text : {
        /* ここで最初にmultibootセクションを結合する */
        KEEP(*(.multiboot))
        *(.text)
    }

    .data : { *(.data) }
    .bss  : { *(.bss) }
}