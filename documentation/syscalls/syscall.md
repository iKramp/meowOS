# SYSCALL-RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | lspacks | Lists mapped packs |
| 1 | lsallpacks | Lists all packs available in the system |
| 2 | mappack | Maps a pack to the process's syscall namespace |
| 3 | unmappack | Unmaps a pack from the process's syscall namespace |
| 4 | restrict | Restricts the syscall useage |

## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: lspacks
#### Args:
1. buf_size: u64 - size of the buffer in count of elements
2. buf_ptr: *mut MappedPackInfo - pointer to a buffer of pack_info structures to be filled by the kernel
#### Return value:
- Returns the number of packs filled into the buffer in return arg 0 and the total number of mapped packs in return arg 1.
#### Description:
Fills the provided buffer with information about the packs currently mapped to the process. 
Each entry is: 
```rust
struct MappedPackInfo {
    pack_info: PackInfo,
    offset: u32,
    mask: u32,
}
````

### Syscall 1: lsallpacks
#### Args:
1. buf_size: u64 - size of the buffer in count of elements
2. buf_ptr: *mut PackInfo - pointer to a buffer of pack_info structures to be filled by the kernel
#### Return value:
- Returns the number of packs filled into the buffer in return arg 0 and the total number of available packs in return arg 1.
#### Description:
Fills the provided buffer with information about all packs available in the system.
Each entry is:
```rust
struct PackInfo {
    name_len: u8,
    name: [u8, 31], //utf8 valid
}
```

### Syscall 2: mappack
#### Args:
1. name_len: u64 - length of the pack name in bytes
2. name_ptr: u64 - pointer to the pack name string (utf8 valid)
3. offset: u32 - offset to map the pack to in the process's syscall namespace
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Maps the pack with the given name to the process's syscall namespace at the specified offset.
The pack name must be valid and available in the system (see lsallpacks).
The offset must be smaller or equal to 2^32 - 32 (to fit all syscalls)
If the same pack is already mapped in the current namespace, restrictions carry over. Otherwise, all syscalls are unrestricted

### Syscall 3: unmappack
#### Args:
1. offset: u64 - offset to unmap the pack from in the process's syscall namespace
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Unmaps the pack at the specified offset from the process's syscall namespace. 
The offset must be smaller than 2^32, and there must be a pack currently mapped at that offset (see lspacks).

### Syscall 4: restrict
#### Args:
1. offset: u64 - offset of the pack to restrict in the process's syscall namespace
1. mask: u64 - bitmask of allowed syscalls
#### Return value:
- On success, returns 0
- On failure, returns -1
#### Description:
Restricts the use of syscalls for the pack at the specified offset in the process's syscall namespace.
The offset must align with a mapped syscall pack (see lspacks), and the mask is a bitmask where each bit represents a syscall number. 
1 represents an allowed syscall and 0 represents a disallowed syscall. This is AND-ed with the current bitmask
