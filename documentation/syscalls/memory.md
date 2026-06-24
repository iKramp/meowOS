# MEMORY SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | make_region | creates a new memory region |
| 1 | remove_region | removes a memory region |
| 2 | list_regions | lists all memory regions in the namespace |
| 3 | set_prot | sets permissions for a region |
| 4 | mmap | maps memory into a region |
| 5 | munmap | unmaps memory from a region |


## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: make_region
#### Args:
1. u64 address - starting address of the region
2. u8  order
3. u8  permissions
4. u8  region_type
5. u64 management_mode
6. u64 region_name_len
7. u64 region_name_ptr - utf8 valid
#### Return Value:
 - On success, returns ID of the created region
 - On failure, returns -1

#### Order:
- 0: 4KB
- 1: 2MB
- 2: 1GB
- 3: 512GB - shouldn't be used much unless you know what you're doing. Memory is limited

> [!WARNING]
> address must be aligned to the size of the region (defined by size order). 
> Requested address will be rounded down to the nearest aligned address, and the region will be created starting from that address.

#### Permissions:
```rust
bitfield! {
    pub struct VirtualMemoryRangePermissions(u8);
    impl Debug;
    pub write, set_write: 0;
    pub execute, set_execute: 1;
}
```

#### Region Type:
```rust
enum MemoryRangeType {
    Stack = 0,
    Code = 1,
    Data = 2,
    Shared = 3,
}
```
This is not used for anything yet. Pls use regardless

#### Management Mode:
Made up of 2 parts:
Lower 32 bits: management mode
 - 0: Managed by the kernel (auto map on page fault, only possible for growing regions)
 - 1: Managed by the userspace (using syscalls for mapping/unmapping memory, allows for non-growing regions)
These modes are exclusive, eg a userspace can't map arbitrary pages in a kernel managed region
Higher 32 bits: Management submode
    - For kernel managed regions:
        - 0: Grow up (most modern heaps)
        - 1: Grow down (most stacks)
    - For userspace managed regions, this is currently unused and should be set to 0


### Syscall 1: remove_region
#### Args:
1. u64 region_id - ID of the region to remove
#### Return Value:
 - On success, returns 0
 - On failure, returns -1

Removes the region with the given ID from the memory namespace. This might not release memory if the same region is used in other namespaces

### Syscall 2: list_regions
TODO

### Syscall 3: set_prot
#### Args:
1. u64 region_id - ID of the region to set permissions for
2. u64 permissions - new permissions for the region (same format as in make_region)
#### Return Value:
 - On success, returns 0
 - On failure, returns -1

### Syscall 4: mmap
#### Args:
1. u64 region_id - ID of the region to map memory into. -1 can be used to specify a global offset (region will be determined by the address). Otherwise, address is relative to the start of the region
2. u64 address - starting address to map memory to (must be aligned to page size, otherwise it will be rounded down)
3. u64 num_pages - number of pages to map
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
Maps num_pages pages of memory into the region with the given ID, starting at the given address. 
The region must have user management mode, and the address must be within the region. The mapped memory will have the same permissions as the region.

### Syscall 5: munmap
#### Args:
1. u64 region_id - ID of the region to unmap memory from. -1 can be used to specify a global offset (region will be determined by the address). Otherwise, address is relative to the start of the region
2. u64 address - starting address to unmap memory from (must be aligned to page size, otherwise it will be rounded down)
3. u64 num_pages - number of pages to unmap
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
Region is determined as described in region_id arg.
If region is user managed:
 - Unmaps num_pages pages of memory from the region with the given ID, starting at the given address.
If region is kernel managed:
 - Removes from (including) address to end of region. End is the farthest the region has grown in whatever direction.
   This is used for shrinking the stack or heap. Region stays contiguous.
 - NOT IMPLEMENTED YET!!!!!
