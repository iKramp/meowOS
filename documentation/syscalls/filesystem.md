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
1. uint64 path_len - length of the path in bytes
1. char* path - path to the file, absolute or relative to current working directory
1. int64 fd - if set and path is relative, it will be relative to fd, not cwd
1. uint64 flags - open mode flags
#### Return Value:
 - On success, returns a non-negative file descriptor
 - On failure, returns -1 and sets errno
#### Flags:
1. flags:
    1. bit 0: READ - allow reading
    1. bit 1: WRITE - allow writing
    1. bit 2: APPEND - append to the end of the file
    1. bit 4: TRUNCATE - truncate the file to zero length
#### Description:
Opens the file at the given path with the specified flags. If the path is absolute, it will go from root.
If it is relative, it will either go from cwd (fd is 0) or from the directory represented by fd.

### Syscall 1: fclose
#### Args:
1. int64 fd - file descriptor to close
#### Return Value:
 - On success, returns 0
 - On failure, returns -1 and sets errno
#### Description:
Closes the given file descriptor, releasing any associated resources and flushing buffers.

### Syscall 2: fread
#### Args:
1. int64 fd - file descriptor to read from
1. uint64 count - number of bytes to read
1. void* buf - buffer to read data into
#### Return Value:
 - On success, returns the number of bytes read. Errno may still be set to indicate additional information, like EOF (in which case the read succeeded, but reached the end.
 - On failure, returns -1 and sets errno
#### Description:
Reads up to count bytes from the file descriptor fd into the buffer buf. The actual number of bytes read may be less than count.

### Syscall 3: fwrite
#### Args:
1. int64 fd - file descriptor to write to
1. uint64 count - number of bytes to write
1. void* buf - buffer containing data to write
#### Return Value:
 - On success, returns the number of bytes written. Errno may still be set to indicate additional information.
 - On failure, returns -1 and sets errno
#### Description:
Writes up to count bytes from the buffer buf to the file descriptor fd. The actual number of bytes written may be less than count.

### Syscall 4: fseek
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
