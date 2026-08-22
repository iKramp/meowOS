# GENERAL SYSCALL DOCUMENTATION

Syscalls in this kernel are made to be flexible. Ideally, there will be no duplicate syscalls (where one just accepts more parametyers)
For example, linux has many syscall() and syscallat() pairs. This creates unnecessary bloat in the syscall table.

## SYSCALL CONVENTIONS

### Linux x86_64 syscall convention
This is used for linux compat syscalls and legacy syscalls

| Arg Number | Register (x86_64) |
|------------|-------------------|
| syscall index | rax |
| 1 | rdi |
| 2 | rsi |
| 3 | rdx |
| 4 | r10 |
| 5 | r8 |
| 6 | r9 |
| Return Value | rax |
| Errno Value | rdx |

This table follows the linux x86_64 convention. 

### Custom convention
This is used for new syscalls that do not need to be linux compatible

| Arg/ret Number | Register (x86_64) |
|------------|-------------------|
| syscall index | rax |
| namespace index | rbx |
| 0 | rdx |
| 1 | rdi |
| 2 | rsi |
| 3 | r8 |
| 4 | r9 |
| 5 | r10 |
| 6 | r12 |
| 7 | r13 |
| 8 | r14 |
| 9 | r15 |

If there are too many parameters, an in process memory structure should be used.
If there are too many return values, an in process memory structure should be used.
All registers are kept, unless they're used as return values

## ERROR HANDLING
Errno value is 0 on success, otherwise it is the error code. Return value may still be valid on error, depending on the syscall.

## STRINGS AND BUFFERS
The kernel uses rust style strings and buffers. They are represented as:
String -> struct where the first field is size (u64) and the second is a pointer to utf8 valid data
String size does NOT include the null terminator, and is measured in bytes (not utf8 codepoints)
```C
struct String {
    uint64_t str_size;
    char *data_ptr;
};
```
Buffer -> struct where the first field is size (u64) in count of elements and the second is a pointer to bytes of data
```C
//pretend rust style generics exist
struct Buffer<T> {
    uint64_t buf_size;
    T *data_ptr;
};
```
For clarity, whenever these values are passed via registers in syscall args (see exec path, arg list and env list) they
will be split

## SYSCALL NAMESPACE
A syscall namespace defines what syscalls a process can use and at what indexes they exist. This is achieved through mapping syscall groups
A syscall group is a group of related syscalls. For example, all filesystem related syscalls would be in the same group. Syscalls within a group
are indexed starting from 0. When a process is created, it is assigned a syscall namespace in which each group gets an "offset"
Specific syscall groups are described in their respective markdown files
Processes started with run_process_default_env (started from kernel shell) are guaranteed to have:
 - legacy syscalls mapped at 0
 - syscall management pack at 0xFFFFFFE0 (highest possible map)

