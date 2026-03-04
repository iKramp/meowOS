use context::{
    builder::create_process,
    info::{ContextInfo, MemoryRegionDescriptor, MemoryRegionFlags},
};
use core::{mem::MaybeUninit, sync::atomic::AtomicU32};
use scheduler::Scheduler;
use std::{
    boxed::Box,
    lock_w_info,
    mem_utils::{PhysAddr, VirtAddr},
    println,
    string::ToString,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::memory::paging::{self, PageTree};

mod context;
mod context_switch;
mod dispatcher;
mod loaders;
mod process_data;
mod scheduler;
mod syscall;
pub use context_switch::{context_switch, interrupt_context_switch};
pub use process_data::{ProcessData, StackCpuStateData};
pub use scheduler::save_and_release_current;

static SCHEDULER: NoIntSpinlock<MaybeUninit<Scheduler>> = NoIntSpinlock::new(MaybeUninit::uninit());

static PROCESS_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

pub static mut PROC_INITIALIZED: bool = false;
static mut GENERIC_PAGE_TREE: PhysAddr = PhysAddr(0);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Pid(pub u32);

/// notes:
/// page tree root should always be unique
/// stack size pages should not be larger than [`context::info::MAX_PROC_STACK_SIZE_PAGES`]
#[derive(Debug)]
pub(super) struct MemoryContext {
    initialized: bool,
    is_32_bit: bool,
    page_tree: PageTree,
    owned_memory_regions: Vec<MappedMemoryRegion>,
    //shared regions here?
}

impl Default for MemoryContext {
    fn default() -> Self {
        Self {
            initialized: false,
            is_32_bit: false,
            page_tree: PageTree::new(PhysAddr(0)),
            owned_memory_regions: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct MappedMemoryRegion {
    name: Box<str>,
    base: VirtAddr,
    size_pages: u64,
}

pub fn init() {
    // Initialize the scheduler
    let mut scheduler = lock_w_info!(SCHEDULER);
    *scheduler = MaybeUninit::new(Scheduler::new());
    drop(scheduler);
    create_fallback_process();
    loaders::init_process_loaders();

    // let prime_finder = loaders::load_process(crate::PRIME_FINDER, "[prime_finder]".to_string().into_boxed_str())
    //     .expect("Failed to load test executable prime finders");
    // for _i in 0..10 {
    //     let pid = create_process(&prime_finder);
    //     println!("Created process with pid: {:?}", pid);
    // }
    //
    // let time_printer = loaders::load_process(crate::TIME_PRINTER, "[time_printer]".to_string().into_boxed_str())
    //     .expect("Failed to load test executable time printer");
    // for _i in 0..10 {
    //     let pid = create_process(&time_printer);
    //     println!("Created process with pid: {:?}", pid);
    // }

    let file_reader = loaders::load_process(crate::FILE_READER, "[file_reader]".to_string().into_boxed_str())
        .expect("Failed to load test executable file reader");
    let pid = create_process(&file_reader);
    println!("Created file reader process with pid: {:?}", pid);


    syscall::init();
    set_proc_initialized();
}

pub fn init_ap() {
    syscall::init();
}

pub fn switch_to_generic_mem_tree() {
    paging::PageTree::set_level4_addr(unsafe { GENERIC_PAGE_TREE });
}

//set this AFTER the process with pid 1 is loaded (pid 0 is fallback, might be removed)
pub fn set_proc_initialized() {
    unsafe {
        GENERIC_PAGE_TREE = paging::PageTree::get_level4_addr();
        PROC_INITIALIZED = true;
    }
}

pub fn get_proc(pid: Pid) -> Option<Arc<ProcessData>> {
    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.get_proc(pid)
}

//for now this only marks the process as stopping. If it was in running state before, return,
//otherwise clear resources
//Also return if it was in stopping state. Reason: stopping means it's either running and has been
//scheduled for stopping (case above), or its resources are actively being freed
pub fn kill_process(pid: Pid, status: u64) {
    let Some(process) = (unsafe { lock_w_info!(SCHEDULER).assume_init_mut().remove_process(pid) }) else {
        return;
    };
    process.set_ret_status(status);
    process.cleanup();
}

pub fn wake_process(pid: Pid) {
    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.wake_proc(pid);
}

//context switch to this process when no other processes exist
pub fn create_fallback_process() {
    let code_region = MemoryRegionDescriptor::new(VirtAddr(0x1000), 1, MemoryRegionFlags(2)).expect("fallback can't error");
    let data_region = MemoryRegionDescriptor::new(VirtAddr(0x2000), 1, MemoryRegionFlags(1)).expect("fallback can't error");
    let code_init = [
        0x90,                  //nop
        0x90,                  //nop
        0x90,                  //nop
        0x90,                  //nop
        0x48,                  //vvv
        0xC7,                  //vvv
        0xC7,                  //mov rdi, imm
        0x01,                  //vvv
        0x00,                  //vvv
        0x00,                  //vvv
        0x00,                  //0x1
        0x48,                  //vvv
        0xC7,                  //vvv
        0xC6,                  //mov rsi, imm
        0x00,                  //vvv
        0x20,                  //vvv
        0x00,                  //vvv
        0x00,                  //0x2000
        0x0f,                  //vvv
        0x05,                  //syscall
        0x90,                  //nop
        0xEB,                  //jmp
        0_u8.wrapping_sub(20), //jmp offset
    ];
    let data_init = b"Message from user process: uhhhh idk something something works??\0";

    let fake_context = ContextInfo::new(
        false,
        &mut [code_region, data_region],
        Box::new([(VirtAddr(0x1000), &code_init), (VirtAddr(0x2000), data_init)]),
        VirtAddr(0x1000),
        "fallback_process".to_string().into_boxed_str(),
        "[fallback_process]".to_string().into_boxed_str(),
    )
    .expect("fallback can't error");
    let pid = create_process(&fake_context).expect("failed to create fallback process");
    assert_eq!(pid.0, 0);
    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.remove_process(pid);
}
