# FILESYSTEM SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | fopen | opens a file and returns a file descriptor |
| 1 | fclose | closes a file descriptor |
| 2 | fread | reads data from a file descriptor |
| 3 | fwrite | writes data to a file descriptor |
| 4 | fseek | seeks to a position in a file descriptor |


## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: fopen
#### Args:
1. u64 path_len - length of the path in bytes
1. *u8 path - path to the file, absolute or relative to current working directory
1. u64 fd - if set and path is relative, it will be relative to fd, not cwd
1. u64 flags - open mode flags
#### Return Value:
 - On success, returns a non-negative file descriptor
 - On failure, returns -1
#### Flags:
```rust
bitfield! {
    struct OpenFlags(u64);
    impl Debug;
    pub read, set_read: 0;
    pub write, set_write: 1;
    pub append, set_append: 2;
    pub truncate, set_truncate: 3;
}
```
#### Description:
Opens the file at the given path with the specified flags. If the path is absolute, it will go from root.
If it is relative, it will either go from cwd (fd is 0) or from the directory represented by fd.

### Syscall 1: fclose
#### Args:
1. u64 fd - file descriptor to close
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Closes the given file descriptor, releasing any associated resources and flushing buffers.

### Syscall 2: fread
#### Args:
1. u64 fd - file descriptor to read from
1. u64 count - number of bytes to read
1. *u8 buf - buffer to read data into
#### Return Value:
 - On success, returns the number of bytes read in the first return arg. Second contains additional information (e.g. EOF)
 - On failure, returns -1
#### Description:
Reads up to count bytes from the file descriptor fd into the buffer buf. The actual number of bytes read may be less than count.

### Syscall 3: fwrite
#### Args:
1. u64 fd - file descriptor to write to
1. u64 count - number of bytes to write
1. *u8 buf - buffer containing data to write
#### Return Value:
 - On success, returns the number of bytes written 
 - On failure, returns -1
#### Description:
Writes up to count bytes from the buffer buf to the file descriptor fd. The actual number of bytes written may be less than count.

### Syscall 4: fseek
#### Args:
1. i64 fd - file descriptor to seek
1. i64 offset - offset to seek to
1. u64 seek_mode
#### Return Value:
 - On success, returns the new offset from the beginning of the file
 - On failure, returns -1
#### seek_mode:
```rust
enum FileSeekMode {
    Start = 0,
    Current = 1,
    End = 2,
}
```
#### Description:
Repositions the file offset of the open file descriptor fd according to the offset and whence parameters.
