section .text

global probe_functions_start
global probe_check_u64
global probe_check_u32
global probe_check_u16
global probe_check_u8
global probe_functions_end
global probe_fail

probe_functions_start:
probe_check_u64:
    mov rax, [rdi]
    mov rdx, 1
    ret

probe_check_u32:
    xor rax, rax
    mov eax, [rdi]
    mov rdx, 1
    ret
probe_check_u16:
    xor rax, rax
    mov ax, [rdi]
    mov rdx, 1
    ret
probe_check_u8:
    xor rax, rax
    mov al, [rdi]
    mov rdx, 1
    ret
probe_functions_end:

probe_fail:
    mov rdx, 0
    ret
