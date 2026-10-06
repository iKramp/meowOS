section .ap_startup
[bits 16]
global ap_startup


ap_startup:
    cli
    cld
    mov ebx, 0
    jmp _ap_start

_PADDING:
    dd 0
_GDT_PTR:  ; passed from BP
    dw 0xabcd
    dq 0
_CR3: dq 0 ; passed from BP
_STACK: dq 0 ; passed from BP
_WAIT_LOOP: dq 0 ; passed from BP
_MTRR_DEF_TYPE: dq 0 ; passed from BP
_COMM_LOCK: db 0
_COMM_DATA_READY: db 0
_COMM_DATA: db 0
_TEMP_GDT:
    dq 0
    dq 0x00_C_F_9A_00_0000_FFFF    ; flat code
    dq 0x00_C_F_92_00_0000_FFFF    ; flat data
    dq 0    ; TSS, we hope we won't need it in real and 32 bit mode
_END_GDT:
_TEMP_GDT_PTR:
    dw (_END_GDT - _TEMP_GDT - 1)
    dd 0
_START_64_ADDR:
    dd 0
    dw 8


_ap_start:
    mov ax, cs
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov eax, ebx
    mov [_TEMP_GDT - ap_startup + 0xa], ax ;set base of code segment
    shr eax, 16
    mov [_TEMP_GDT - ap_startup + 0xc], al ;set base of data segment

    mov eax, ebx
    add eax, _TEMP_GDT - ap_startup ; load GDT
    mov [(_TEMP_GDT_PTR - ap_startup + 2)], eax
    lgdt [(_TEMP_GDT_PTR - ap_startup)]

    mov eax, cr0 ;enable protected mode
    or eax, 1
    mov cr0, eax
    
    jmp 0x8:(_ap_start32 - ap_startup)

[bits 32]
_ap_start32:
    mov eax, dword [_CR3 - ap_startup] ; Grab CR3
    mov cr3, eax

    mov eax, cr4
    or eax, 1 << 5     ; Set the PAE bit
    mov cr4, eax

    ; Set LME (long mode enable)
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    or eax, (1 << 11) ; set execute disable bit enable
    wrmsr

    ; enable paging
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    ; refresh paging
    mov eax, cr3
    mov cr3, eax

    mov eax, ebx
    add eax, _ap_start64 - ap_startup
    mov [_START_64_ADDR - ap_startup], eax

    lgdt [_GDT_PTR - ap_startup]
    jmp far [_START_64_ADDR - ap_startup]

[bits 64]
_ap_start64:
    ; set up stack
    mov rsp, [rbx + _STACK - ap_startup]
    mov rax, 0

    ; refresh paging
    mov rax, qword [rbx + _CR3 - ap_startup] ; Grab CR3
    mov cr3, rax

    mov ecx, 0xC000_0080
    rdmsr

    push 0x08
    lea rax, [rel _ret_addr]
    push rax
    retfq
_ret_addr:
    mov ax, 0x10 ;data segment
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    ;set PAT
    mov rcx, 0x277
    rdmsr
    and eax, 0xffff00ff
    or eax, 0x00000100
    wrmsr

    ;enable SSE
    mov rax, cr0
    and ax, 0xFFFB
    or ax, 0x2
    mov cr0, rax
    mov rax, cr4
    or ax, 3 << 9
    mov cr4, rax

    push 0
    push 0

    mov rdi, rbx
    add rdi, _COMM_LOCK - ap_startup
    push rdi

    cld

    mov rax, [rbx + _WAIT_LOOP - ap_startup]
    call rax
