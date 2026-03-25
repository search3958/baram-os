BITS 32
section .text
global setjmp
global longjmp

setjmp:
    mov edx, [esp + 4] ; jmp_buf pointer
    mov [edx], ebx
    mov [edx + 8], esi
    mov [edx + 16], edi
    mov [edx + 24], ebp
    
    ; Stack pointer as it will be after 'ret'
    lea ecx, [esp + 4]
    mov [edx + 32], ecx
    
    ; Return address (eip)
    mov ecx, [esp]
    mov [edx + 40], ecx
    
    xor eax, eax
    ret

longjmp:
    mov edx, [esp + 4] ; jmp_buf pointer
    mov eax, [esp + 8] ; val
    test eax, eax
    jnz .non_zero
    inc eax
.non_zero:
    mov ebx, [edx]
    mov esi, [edx + 8]
    mov edi, [edx + 16]
    mov ebp, [edx + 24]
    mov esp, [edx + 32]
    jmp [edx + 40]
