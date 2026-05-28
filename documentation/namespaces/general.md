# GENERAL NAMESPACE DOCUMENTATION

Each process has an environment of namespaces such as syscall, fs, net, mem,...
These namespaces can be exclusive or shared, and are used for any interaction with the environment via syscalls
Current namespace types and their enum values are:

```rust
#[repr(u32)]
enum NamespaceType {
    Syscall = 0,
    Mem = 1,
    Fs = 2,
}
```
