# FILESYSTEM SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | fopen | opens a file and returns a file descriptor |
| 1 | fclose | closes a file descriptor |
| 2 | fread | reads data from a file descriptor |
| 3 | fwrite | writes data to a file descriptor |
| 4 | fseek | seeks to a position in a file descriptor |
| 5 | fcreate | creates a new file and returns a file descriptor |
| 6 | flink | creates a hard link to a file |
| 7 | funlink | unlinks a file |
| 8 | fstat | gets information about a file descriptor |


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
1. u64 flags - read mode flags (ReadModeFlags)
#### Return Value:
 - On success, returns the number of bytes read in the first return arg.
 - On failure, returns -1
 - Second return arg specifies the current file state according to enum ReadFileState
#### ReadFileState:
```rust
enum ReadFileState {
    Normal = 0,
    TemporaryEOF = 1, //normal files, net sockets and pipes while peer is connected
    PermanentEOF = 2, //net sockets and pipes when peer disconnects
}
```
#### ReadModeFlags:
```rust
bitfield! {
    struct ReadModeFlags(u64);
    impl Debug;
    pub nonblocking, set_nonblocking: 0;
}
```
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

### Syscall 5: fcreate
#### Args:
1. u64 name_len - length of the name in bytes
1. *u8 name - name (direntry name) of the file
1. u64 parent_fd - file descriptor of the parent directory to create the file in
1. u64 type - type of the inode to be created
1. u64 permissions - permissions for the new file
#### Return Value:
 - On success, returns 0
 - On failure, returns -1

#### Type:
```rust
pub enum InodeType {
    File = 0,        //--\
    Directory = 1,   //------real file types
    Symlink = 2,     //--/
    Socket = 3,      //--\
    BlockDevice = 4, //---\
    CharDevice = 5,  //------mental illnesses
    Fifo = 6,        //---/
}
```

#### Permissions:
```rust
bitfield! {
    pub struct InodePermissionFlags(u32);
    impl Debug;
    pub suid, set_suid: 11;
    pub sgid, set_sgid: 10;
    pub sticky, set_sticky: 9;

    pub r_usr, set_r_usr: 8;
    pub w_usr, set_w_usr: 7;
    pub x_usr, set_x_usr: 6;

    pub r_grp, set_r_grp: 5;
    pub w_grp, set_w_grp: 4;
    pub x_grp, set_x_grp: 3;

    pub r_othr, set_r_othr: 2;
    pub w_othr, set_w_othr: 1;
    pub x_othr, set_x_othr: 0;
}
```
#### Description:
Creates a new file with the specified name, type, and permissions in the directory represented by parent_fd. 
The name is a direntry name, not any kind of path.

### Syscall 6: flink
#### Args:
1. u64 name_len - length of the name in bytes
1. *u8 name - name (direntry name) of the link to be created
1. u64 parent_fd - file descriptor of the directory to create the link in
1. u64 target_fd - file descriptor of the file to link to
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Creates a hard link with the specified name in the directory represented by parent_fd that points to the same file as target_fd. The name is a direntry name, not any kind of path.

### Syscall 7: funlink
#### Args:
1. u64 fd - file descriptor of the directory in which the file to be deleted is located
1. u64 name_len - length of the name in bytes
1. *u8 name - name (direntry name) of the file to be deleted
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Deletes the file with the specified name in the directory represented by fd. The name is a direntry name, not any kind of path.
In reality this calls unlink on the filesystem, so if the file has multiple hard links, it won't be removed yet.

### Syscall 8: fstat
#### Args:
1. u64 fd - file descriptor to get information about
1. u64 buf - buffer to write the stat struct to
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Gets information about the file descriptor fd and writes it to the buffer buf as a stat struct.
#### Stat struct:
```rust
#[repr(C)]
pub struct Inode {
    pub index: u64,
    pub device: u64,
    pub type_mode: InodeTypeAndPerms,
    pub link_cnt: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u64,
    pub access_time: u64,
    pub modification_time: u64,
    pub stat_change_time: u64,
}

///The top 8 bits represent the file type [`InodeType`] (bit shifted)
///The bottom 24 bits represent [`InodePermissionFlags`]
pub struct InodeTypeAndPerms(u32);
```
