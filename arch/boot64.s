BITS 32
DEFAULT REL

%define CR0_PG      0x80000000
%define CR4_PAE     0x00000020
%define EFER_MSR    0xC0000080
%define EFER_LME    0x00000100
%define PAGE_PRESENT 0x01
%define PAGE_RW      0x02
%define PAGE_PS      0x80
%define PD_TABLE_COUNT 4

section .multiboot
align 4
    dd 0x1BADB002
    dd 0x00000007
    dd -(0x1BADB002 + 0x00000007)
    dd 0, 0, 0, 0, 0
    dd 0
    dd 1280
    dd 720
    dd 32

section .text
global _start
extern kmain

_start:
    cli
    mov [multiboot_magic], eax
    mov [multiboot_info], ebx
    mov esp, stack32_top

    call setup_page_tables

    mov eax, pml4_table
    mov cr3, eax

    mov eax, cr4
    or eax, CR4_PAE
    mov cr4, eax

    mov ecx, EFER_MSR
    rdmsr
    or eax, EFER_LME
    wrmsr

    mov eax, cr0
    or eax, CR0_PG
    mov cr0, eax

    lgdt [gdt64_ptr]
    jmp 0x08:long_mode_start

setup_page_tables:
    mov edi, pml4_table
    mov ecx, (4096 * (2 + PD_TABLE_COUNT)) / 4
    xor eax, eax
    rep stosd

    mov eax, pdpt_table
    or eax, PAGE_PRESENT | PAGE_RW
    mov [pml4_table], eax

    xor ecx, ecx
.setup_pdpt:
    mov eax, pd_tables
    mov edx, ecx
    shl edx, 12
    add eax, edx
    or eax, PAGE_PRESENT | PAGE_RW
    mov [pdpt_table + ecx * 8], eax
    mov dword [pdpt_table + ecx * 8 + 4], 0
    inc ecx
    cmp ecx, PD_TABLE_COUNT
    jne .setup_pdpt

    xor ecx, ecx
.map_pd:
    mov eax, ecx
    shl eax, 21
    or eax, PAGE_PRESENT | PAGE_RW | PAGE_PS
    mov edx, ecx
    shr edx, 9
    shl edx, 12
    mov ebx, ecx
    and ebx, 0x1FF
    mov [pd_tables + edx + ebx * 8], eax
    mov dword [pd_tables + edx + ebx * 8 + 4], 0
    inc ecx
    cmp ecx, (512 * PD_TABLE_COUNT)
    jne .map_pd
    ret

BITS 64
long_mode_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov rsp, stack64_top

    mov edi, dword [multiboot_magic]
    mov esi, dword [multiboot_info]
    call kmain

.halt:
    hlt
    jmp .halt

section .data
align 8
gdt64:
    dq 0
    dq 0x00AF9A000000FFFF
    dq 0x00CF92000000FFFF
gdt64_end:

gdt64_ptr:
    dw gdt64_end - gdt64 - 1
    dq gdt64

section .bss
alignb 16
multiboot_magic: resd 1
multiboot_info:  resd 1

alignb 4096
pml4_table: resb 4096
alignb 4096
pdpt_table: resb 4096
alignb 4096
pd_tables:  resb (4096 * PD_TABLE_COUNT)

alignb 16
stack32_bottom: resb 65536
stack32_top:

alignb 16
stack64_bottom: resb 65536
stack64_top:
