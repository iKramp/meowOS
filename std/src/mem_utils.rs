static mut MEM_INITIALIZED: bool = false;
static mut HEAP_INITIALIZED: bool = false;

pub fn set_heap_initialized() {
    unsafe { HEAP_INITIALIZED = true };
}

pub fn get_heap_initialized() -> bool {
    unsafe { HEAP_INITIALIZED }
}
