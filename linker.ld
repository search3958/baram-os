/* x86_64用リンカスクリプト */

ENTRY(kernel_main)

SECTIONS
{
    . = 0x100000; /* 1MB にロード */
    
    .text : ALIGN(4K)
    {
        *(.text*)
    }
    
    .rodata : ALIGN(4K)
    {
        *(.rodata*)
    }
    
    .data : ALIGN(4K)
    {
        *(.data*)
    }
    
    .bss : ALIGN(4K)
    {
        __bss_start = .;
        *(COMMON)
        *(.bss*)
        __bss_end = .;
    }
    
    __kernel_end = .;
}
