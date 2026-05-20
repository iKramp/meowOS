# EXEC RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST

| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | exec | executes a new program |

## STRUCTURE INFORMATION

```rust
#[repr(C)]
struct Namespaces {
    memory_namespace: u64,
    syscall_namespace: u64,
}

//This changes based on the architecture
#[repr(C)]
struct X86RegisterState {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rsp: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

#[repr(C)]
union RegisterState {
    x86: X86RegisterState,
    //other architectures here
}

#[repr(C)]
struct ExecArgs {
    namespaces: Namespaces,
    registers: RegisterState,
    start_ptr: u64,
    name_len: u64,
    name_ptr: u64,
}
```

## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: exec
#### Args:
1. exec_args_ptr: u64 - pointer to an ExecArgs structure containing the arguments for the exec syscall
#### Return Value:
 - On success, returns child PID
 - On failure, returns -1
#### Description:
Executes a new program with the environment defined in the Namespaces and RegisterState structures.
The program starts executing at start_ptr
namespace id 0 means clone current namespace
