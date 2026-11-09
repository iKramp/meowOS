# TIME RELATED SYSCALL DOCUMENTATION

## SYSCALL LIST
| Syscall Number | Name | Description |
|----------------|------|-------------|
| 0 | time | retrieves various time related info |
| 1 | set_time_off | changes the time offset for the time namespace |


## STRUCTURE INFORMATION

```rust
#[repr(C)]
struct TimeInfo {
    time_since_epoch: FfiDuration,
    time_since_boot: FfiDuration,
    program_start_time: FfiDuration,
    program_userspace_cpu_time: FfiDuration,
    program_kernelspace_cpu_time: FfiDuration,
}
```

### Syscall 0: time
#### Args:
1. time_info_ptr: u64 - pointer to a TimeInfo structure where the time information will be stored
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Retrieves various time-related information, including time since epoch, time since boot, 
program start time, and CPU time spent in user and kernel space. 
The information is stored in the provided TimeInfo structure.
Fails if the pointer is invalid

### Syscall 1: set_time_off
#### Args:
1. monotonic_offset-high: i128-high - the new offset to be applied to the monotonic clock
1. monotonic_offset-low: i128-low - the new offset to be applied to the monotonic clock
1. boot_time_offset-high: i128-high - the new offset to be applied to the boot time clock
1. boot_time_offset-low: i128-low - the new offset to be applied to the boot time clock
#### Return Value:
 - On success, returns 0
 - On failure, returns -1
#### Description:
Changes the time offset for the time namespace. The new offset is added to the current offset.
monotonic_time_offset -> changes the appparent amount of time passed since unix epoch (larger offset -> later time)
boot_time_offset -> changes the appparent amount time passed since boot (larger offset -> more time since boot/kernel "booted" earlier)
As such, the current offset can never be retrieved (isolation).
A new "freshly booted" container namespace can be created by reading current boot time from time syscall and
creating a new time namespace with boot offset set to negative of that value
Fails if either time would return a negative value on time syscall
