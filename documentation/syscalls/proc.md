# PROC RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST

| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | exec | executes a new program |
| 1 | exit | terminates the current process |
| 2 | sleep | puts the process to sleep for a specified duration |

## STRUCTURE INFORMATION

```rust
#[repr(C)]
struct NamespaceIds {
    memory_namespace: u64,
    syscall_namespace: u64,
    filesystem_namespace: u64,
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
    namespaces: NamespaceIds,
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

### Syscall 1: exit
#### Args:
1. exit_code: u64 - the exit code of the process, which can be retrieved by the parent process
#### Return Value:
 - This syscall does not return a value
#### Description:
Terminates the current process and returns the given exit code to the parent process

### Syscall 2: sleep
#### Args:
1. duration_sec: u64 - the duration to sleep in seconds
1. duration_nsec: u64 - the additional duration to sleep in nanoseconds
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Puts the process to sleep for the specified duration
