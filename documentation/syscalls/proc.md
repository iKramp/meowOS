# PROC RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST

| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | exec | executes a new program |
| 1 | exit | terminates the current process |
| 2 | sleep | puts the process to sleep for a specified duration |
| 3 | getpid | retrieves the process ID of the current process |
| 4 | set_sig_handler | sets a signal handler for a specific signal |
| 5 | remove_sig_handler | removes a signal handler for a specific signal (go back to default action) |
| 6 | send_signal | sends a signal to a specific process |

## STRUCTURE INFORMATION

```rust
#[repr(C)]
struct NamespaceIds {
    memory_namespace: u64,
    syscall_namespace: u64,
    open_file_namespace: u64,
    time_namespace: u64,
    mount_namespace: u64,
}

//This changes based on the architecture
#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct RegisterState {
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
struct ExecArgs {
    namespaces: NamespaceIds,
    registers: RegisterState,
    start_ptr: u64,
    name_len: u64,
    name_ptr: u64,
    clone_ns_table: bool,
}

#[repr(C)]
struct FfiDuration {
    sec: u64,
    nsec: u64,
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
Namespace id 0 means clone current namespace
If clone_ns_table is false, a new table is constructed from namespaces used. Otherwise, it is cloned from the current process

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

### Syscall 3: getpid
#### Args:
#### Return Value:
 - Returns the process ID of the current process
#### Description:
Retrieves the process ID of the current process

### Syscall 4: set_sig_handler
#### Args:
1. signal_number: u64 - the signal number for which to set the handler
1. handler_ptr: u64 - pointer to the signal handler function
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Sets a signal handler for a specific signal. The handler function will be called when the specified signal is received by the process

### Syscall 5: remove_sig_handler
#### Args:
1. signal_number: u64 - the signal number for which to remove the handler
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Removes a signal handler for a specific signal, reverting to the default action for that signal

### Syscall 6: send_signal
#### Args:
1. target_pid: u64 - the process ID of the target process to which the signal will be sent
1. signal_number: u64 - the signal number to send to the target process
1. sig_data_len: u64 - length of the additional data to send with the signal (max 128B)
1. sig_data: *const u8 - pointer to additional data to send with the signal
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Sends a signal to a specific process. Data can be sent (where appropriate) with the signal. This is not supported for
linux signals
