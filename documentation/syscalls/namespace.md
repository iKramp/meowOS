# NAMESPACE SYSCALL DOCUMENTATION

## GENERAL INFO
Each process has an environment of namespaces such as syscall, fs, net, mem,...
A syscall can at any moment create a namespace, switch its operating namespace, or destroy a namespace.
Some namespaces may be shared between processes

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | mknamespace | Creates a new namespace of the given type and returns its ID |
| 1 | rmnamespace | Destroys the namespace with the given ID |
| 2 | chnamespace | Sets the process's current namespace of the type to the given ID |
| 3 | lsnamespace | Lists the namespaces owned by the process |
| 4+ | future | sending and receiving namespaces |

## DETAILED SYSCALL DOCUMENTATION

### Syscall 0: mknamespace
#### Args:
1. type: u64 - type of the namespace to create (defined in documentation/namespaces/general.md)
1. existing_id: u64 - if 0, an empty namespace is created. If non-zero, the new namespace is initialized as a copy of the existing namespace with the given ID. The process must be an owner of the existing namespace.
#### Return Value:
 - On success, returns the ID of the created namespace
 - On failure, returns -1
#### Description:
Creates a new namespace of the given type and returns its ID. The process becomes the owner of the namespace and can manage it.

### Syscall 1: rmnamespace
#### Args:
1. id: u64 - ID of the namespace to destroy
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Destroys the namespace with the given ID. The process must be the owner of the namespace to destroy it, and must not
currently be using it. If it's a shared namespace, it will only be removed from this process

### Syscall 2: chnamespace
#### Args:
1. id: u64 - ID of the namespace to switch to
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Sets the process's current namespace of the same type as id to the namespace identified by id. The process must be an owner of the namespace.

### Syscall 3: lsnamespace
#### Args:
1. buf: u64 - pointer to a buffer of namespace_info structures to be filled by the kernel
2. buf_size: u64 - size of the buffer in count of elements
#### Return Value:
 - On success, returns the number of namespaces filled into the buffer in the first return arg and total number of namespaces in the second.
 - On failure, returns -1
#### Description:
Fills the provided buffer with information about the namespaces owned by the process. Each entry is:
```rust
#[repr(C)]
struct NamespaceInfo {
    id: u64,
    type: NamespaceType, //32 bit
    currently_used: bool;
}
```
