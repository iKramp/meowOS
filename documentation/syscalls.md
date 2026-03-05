# GENERAL SYSCALL DOCUMENTATION

Syscalls in this kernel are made to be flexible. Ideally, there will be no duplicate syscalls (where one just accepts more parametyers)
For example, linux has many syscall() and syscallat() pairs. This creates unnecessary bloat in the syscall table.

## SYSCALL CONVENTIONS

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
If there are too many parameters for 6 registers, an in process memory structure should be used.
If there are too many return values, an in process memory structure should be used.

## ERROR HANDLING
Errno value is 0 on success, otherwise it is the error code. Return value may still be valid on error, depending on the syscall.

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | reserved | should not be used |
| 1 | exit | terminates the calling process |
| 2 | exec | spawns a *new* process |
| 3 | clone | creates a new thread in the calling process |
| 4 | fopen | opens a file and returns a file descriptor |
| 5 | fclose | closes a file descriptor |
| 6 | fread | reads data from a file descriptor |
| 7 | fwrite | writes data to a file descriptor |
| 8 | fseek | seeks to a position in a file descriptor |
| 9 | mmap | maps a file or device into memory |
| 10 | munmap | unmaps a mapped region of memory |
| 11 | sleep | puts the calling process to sleep for a specified duration |
| 12 | time | gets the current system time |

This table will be expanded

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

## DETAILED SYSCALL DOCUMENTATION

### Syscall 1: exit
#### Args:
1. uint64 status - exit status code
#### Description:
 - Terminates the calling process with the given status code. Any children are also terminated (sub-threads)

### Syscall 2: exec
#### Args:
1. uint64 path_len - length of the path in bytes
1. char*  path - path to the executable
1. EnvData *env - pointer to struct describing the environment. Nullptr means default environment
#### Return Value:
 - On success, returns the PID of the new process.
 - On failure, returns -1 and sets errno.
### EnvData struct:
```C
struct EnvData {
    Buffer<String> args; // list of arguments to pass to the new process
    Buffer<String> env_vars; // list of environment variables in the form "KEY=VALUE"
    bool copy_fd;
    bool copy_stdio;
    bool paused; //start the process paused. This allows the caller to modify its environment further
};

```
#### Description:  
Spawns a new process by loading and executing the binary at the given path with the provided environment.
Returns the PID of the new process on success. Unlike linux fork + execve combo, this does NOT create a copy of the calling process.

### Syscall 3: clone
#### Args:
1. uint64 flags - clone flags (idk yet, but it's here)
#### Return value:
 - On success, PID of the new process in the existing process, 0 in the new process
 - On failure, -1 and sets errno
#### Flags argument:
1. bit 0: CLONE_MEM - if set, the new process has a clone of the memory space instead of sharing it
1. bit 1: NO_FD - if set, the new process does not inherit open file descriptors (except for standard in/out/err)
1. bit 2: NO_STDIO - if set, the new process does not inherit standard input/output/error
#### Description:  
Clones the current process. The new "environment" is identical to the old one, but flags dictates what should be shared and what separate

### Syscall 4: fopen
#### Args:
1. uint64 path_len - length of the path in bytes
1. char* path - path to the file, absolute or relative to current working directory
1. int64 fd - if set and path is relative, it will be relative to fd, not cwd
1. uint64 flags - open mode flags
1. uint64 create_mode - file creation mode
#### Return Value:
 - On success, returns a non-negative file descriptor
 - On failure, returns -1 and sets errno
#### Flags:
1. flags:
    1. bit 0: READ - allow reading
    1. bit 1: WRITE - allow writing
    1. bit 2: APPEND - append to the end of the file
    1. bit 3: CREATE - create the file if it does not exist
    1. bit 4: TRUNCATE - truncate the file to zero length if it exists
2. create_mode:
    1. bit 0: USER_READ - user read permission
    1. bit 1: USER_WRITE - user write permission
    1. bit 2: USER_EXECUTE - user execute permission
    1. bit 3: GROUP_READ - group read permission
    1. bit 4: GROUP_WRITE - group write permission
    1. bit 5: GROUP_EXECUTE - group execute permission
    1. bit 6: OTHER_READ - other read permission
    1. bit 7: OTHER_WRITE - other write permission
    1. bit 8: OTHER_EXECUTE - other execute permission
    1. bit 9: STICKY - sticky bit - same as linux for directories
    1. bit 10: SETUID - set user ID on execution
    1. bit 11: SETGID - set group ID on execution
    1. bit 12: DIRECTORY - create as a directory
#### Description:
Opens the file at the given path with the specified flags. If the path is absolute, it will go from root.
If it is relative, it will either go from cwd (fd is 0) or from the directory represented by fd.
The fd has to be currently open if used, as a permission check.

### Syscall 5: fclose
#### Args:
1. int64 fd - file descriptor to close
#### Return Value:
 - On success, returns 0
 - On failure, returns -1 and sets errno
#### Description:
Closes the given file descriptor, releasing any associated resources and flushing buffers.

### Syscall 6: fread
#### Args:
1. int64 fd - file descriptor to read from
1. uint64 count - number of bytes to read
1. void* buf - buffer to read data into
#### Return Value:
 - On success, returns the number of bytes read. Errno may still be set to indicate additional information, like EOF (in which case the read succeeded, but reached the end.
 - On failure, returns -1 and sets errno
#### Description:
Reads up to count bytes from the file descriptor fd into the buffer buf. The actual number of bytes read may be less than count.

### Syscall 7: fwrite
#### Args:
1. int64 fd - file descriptor to write to
1. uint64 count - number of bytes to write
1. void* buf - buffer containing data to write
#### Return Value:
 - On success, returns the number of bytes written. Errno may still be set to indicate additional information.
 - On failure, returns -1 and sets errno
#### Description:
Writes up to count bytes from the buffer buf to the file descriptor fd. The actual number of bytes written may be less than count.

### Syscall 8: fseek
#### Args:
1. int64 fd - file descriptor to seek
1. int64 offset - offset to seek to
1. uint64 whence - seek mode
#### Return Value:
 - On success, returns the new offset from the beginning of the file
 - On failure, returns -1 and sets errno
#### Whence values:
1. SEEK_SET(0) - set the offset to offset bytes from the beginning
1. SEEK_CUR(1) - set the offset to current location plus offset
1. SEEK_END(2) - set the offset to the size of the file plus offset
#### Description:
Repositions the file offset of the open file descriptor fd according to the offset and whence parameters.

### Syscall 9: mmap
#### Args:
1. int64 fd - file descriptor to map (or -1 for anonymous mapping)
1. uint64 offset - offset in the file to start the mapping
1. void* addr - desired starting address for the mapping (can be NULL)
1. uint64 size - length of the mapping in bytes, capped at size of fd (so -1 for whole file)
1. uint64 flags - mapping flags
#### Return Value:
 - On success, returns the starting address of the mapped area
 - On failure, returns NULL and sets errno
#### Flags:
1. bit 0: READ - pages may be read
1. bit 1: WRITE - pages may be written
1. bit 2: EXECUTE - pages may be executed
1. bit 3: CLEAR - pages are zeroed on mapping
#### Description:
Maps a file or device into memory. If fd is -1, an anonymous mapping is created. The mapping starts at the specified offset in the file and spans size bytes.
The addr parameter can be used to suggest a starting address for the mapping; if NULL, the kernel chooses the address.
The flags parameter specifies the desired memory protection of the mapping.

### Syscall 10: munmap
#### Args:
1. void* addr - starting address of the mapping to unmap
1. uint64 size - length of the mapping in bytes
#### Return Value:
 - On success, returns 0. If part of the region is not mapped but unmap is still successful, errno is set along with 0 in return value.
 - On failure, returns -1 and sets errno
#### Description:
Unmaps a previously mapped region of memory starting at addr and spanning size bytes. This releases the mapping and any associated resources.

### Syscall 11: sleep
#### Args:
1. uint64 microseconds - duration to sleep in microseconds
#### Return Value:
 - On success, returns 0
 - On failure, returns -1 and sets errno
#### Description:
Puts the calling process to sleep for at least the specified duration in microseconds. Resolution of the sleep is up to the kernel (might be rounded up to the nearest 100 micros). Sleep of 0 microseconds just yields to the kernel and does not invoke any kind of sleep.

### Syscall 12: time
#### Args:
1. uint64* seconds - pointer to store the current time in seconds since epoch
1. uint64* nanoseconds - pointer to store the current time in nanoseconds part
#### Return Value:
 - On success, returns 0
 - On failure, returns -1 and sets errno
#### Description:
Retrieves the current system time. The time is returned in seconds and nanoseconds since the Unix epoch (January 1, 1970). The seconds and nanoseconds are stored in the provided pointers.

