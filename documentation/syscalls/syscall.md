# SYSCALL-RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | lsgroups | Lists mapped groups |
| 1 | lsallgroups | Lists all groups available in the system |
| 2 | mapgroup | Maps a group to the process's syscall namespace |
| 3 | unmapgroup | Unmaps a group from the process's syscall namespace |
| 4 | restrict | Restricts the syscall useage |

## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: lsgroups
#### Args:
1. buf_size: u64 - size of the buffer in count of elements
2. buf_ptr: u64 - pointer to a buffer of group_info structures to be filled by the kernel
#### Return value:
- Returns the number of groups filled into the buffer in return arg 0 and the total number of mapped groups in return arg 1.
#### Description:
Fills the provided buffer with information about the groups currently mapped to the process. 
Each entry is: 
```rust
struct GroupInfo {
    name_len: u8,
    name: [u8, 31], //utf8 valid
    offset: u32,
    mask: u32,
}
````

### Syscall 1: lsallgroups
#### Args:
1. buf_size: u64 - size of the buffer in count of elements
2. buf_ptr: u64 - pointer to a buffer of group_info structures to be filled by the kernel
#### Return value:
- Returns the number of groups filled into the buffer in return arg 0 and the total number of available groups in return arg 1.
#### Description:
Fills the provided buffer with information about all groups available in the system.
Each entry is:
```rust
struct GroupInfo {
    name_len: u8,
    name: [u8, 31], //utf8 valid
    offset: u32,
    mask: u32,
}
```

### Syscall 2: mapgroup
#### Args:
1. name_len: u64 - length of the group name in bytes
2. name_ptr: u64 - pointer to the group name string (utf8 valid)
3. offset: u64 - offset to map the group to in the process's syscall namespace
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Maps the group with the given name to the process's syscall namespace at the specified offset.
The offset must be smaller than 2^32, and the group name must be valid and available in the system (see lsallgroups).
If the same group is already mapped in the current namespace, restrictions carry over. Otherwise, all syscalls are unrestricted

### Syscall 3: unmapgroup
#### Args:
1. offset: u64 - offset to unmap the group from in the process's syscall namespace
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Unmaps the group at the specified offset from the process's syscall namespace. 
The offset must be smaller than 2^32, and there must be a group currently mapped at that offset (see lsgroups).

### Syscall 4: restrict
#### Args:
1. offset: u64 - offset of the group to restrict in the process's syscall namespace
1. mask: u64 - bitmask of allowed syscalls
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Restricts the use of syscalls for the group at the specified offset in the process's syscall namespace.
The offset must align with a mapped syscall group (see lsgroups), and the mask is a bitmask where each bit represents a syscall number. 
1 represents an allowed syscall and 0 represents a disallowed syscall. This is AND-ed with the current bitmask
