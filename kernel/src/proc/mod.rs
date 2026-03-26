use context::builder::create_process;
use core::{mem::MaybeUninit, sync::atomic::AtomicU32};
use scheduler::Scheduler;
use std::{
    boxed::Box,
    error::ErrorCode,
    lock_w_info,
    mem_utils::{PhysAddr, VirtAddr},
    println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::{
    memory::{
        PAGE_TREE_ALLOCATOR,
        paging::{self, PageTree},
        physical_allocator,
    },
    vfs::{self, ResolvedPathBorrowed, file::FileFlags},
};

mod context;
mod context_switch;
mod dispatcher;
mod loaders;
mod namespaces;
mod process_data;
mod scheduler;
mod syscall;
pub use context::CommandSplitter;
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
    loaders::init_process_loaders();

    // let time_printer = loaders::load_process(crate::TIME_PRINTER, "time_printer")
    //     .expect("Failed to load test executable time printer");
    // let pid = create_process(&time_printer);
    // for _i in 0..10 {
    //     let pid = create_process(&time_printer);
    //     println!("Created process with pid: {:?}", pid);
    // }

    // let file_reader = loaders::load_process(crate::FILE_READER, "[file_reader]".to_string().into_boxed_str())
    //     .expect("Failed to load test executable file reader");
    // let pid = create_process(&file_reader);
    // println!("Created file reader process with pid: {:?}", pid);
    //
    // let proc_task = async move {
    //     let path = vfs::resolve_path("/time_printer");
    //     let root_path = vfs::resolve_path("/");
    //     let mut root = vfs::open_file((&root_path).into(), None, FileFlags::new().with_read(true).with_write(true)).await.expect("failed to open root directory");
    //     vfs::create_file(&mut root, "time_printer", InodeType::new_file(0o777)).await.expect("failed to create time printer file");
    //     println!("created file for time printer");
    //     let mut file_handle = vfs::open_file((&path).into(), None, FileFlags::new().with_read(true).with_write(true)).await.expect("failed to open time printer file");
    //
    //     let size_pages = crate::TIME_PRINTER.len().div_ceil(4096) as u64;
    //     let phys_buf = physical_allocator::allocate_contiguius_high(size_pages);
    //     let buf = unsafe { PAGE_TREE_ALLOCATOR.allocate_contigious(size_pages, Some(phys_buf), false) };
    //     let phys_buf_vec = (0..size_pages).map(|i| phys_buf + i * 4096).collect::<Vec<_>>();
    //     unsafe { core::ptr::copy_nonoverlapping(crate::TIME_PRINTER.as_ptr(), buf.0 as *mut u8, crate::TIME_PRINTER.len()) };
    //     println!("allocated phys buffer for time printer file at phys addr: {phys_buf:?}, size: {size_pages} pages");
    //
    //     let res = vfs::write_file(&mut file_handle, &phys_buf_vec, crate::TIME_PRINTER.len() as u64).await.expect("failed to write to time printer file");
    //     println!("wrote time printer file content to file. Wrote {} bytes", res);
    //
    //     //test
    //     let res = vfs::stat_file(&file_handle).await.expect("failed to stat time printer file");
    //     println!("stat printer file: {:?}", res);
    //     close_file(file_handle).await;
    // };
    //
    // task_runner::block_task(Box::pin(proc_task));

    syscall::init();
    set_proc_initialized();
}

pub fn init_ap() {
    syscall::init();
}

pub async fn run_process_default_env(path: ResolvedPathBorrowed<'_>, cmdline: &str) -> Result<Pid, ErrorCode> {
    let mut file_handle = vfs::open_file(path, None, FileFlags::new().with_read(true)).await?;
    let res = vfs::stat_file(&file_handle);
    let stat = match res.await {
        Err(e) => {
            vfs::close_file(file_handle).await;
            return Err(e);
        }
        Ok(stat) => stat,
    };
    let buf_pages = stat.size.div_ceil(4096);
    let phys_buf = physical_allocator::allocate_contiguius_high(buf_pages);
    let buf = unsafe { PAGE_TREE_ALLOCATOR.allocate_contigious(buf_pages, Some(phys_buf), false) };
    let phys_buf_vec = (0..buf_pages).map(|i| phys_buf + i * 4096).collect::<Vec<_>>();
    let read_res = vfs::read_file(&mut file_handle, &phys_buf_vec, stat.size).await?;
    vfs::close_file(file_handle).await;
    if read_res != stat.size {
        for i in 0..buf_pages {
            unsafe { PAGE_TREE_ALLOCATOR.deallocate(buf + i * 4096) };
        }
        return Err(ErrorCode::InternalFSError);
    }

    let context = match loaders::load_process_context(
        unsafe { core::slice::from_raw_parts(buf.0 as *const u8, stat.size as usize) },
        cmdline,
    ) {
        Ok(context) => context,
        Err(e) => {
            println!(level:error, "Failed to load process from file: {}", path.to_string());
            println!(level:error, "Error: {:?}", e);
            for i in 0..buf_pages {
                unsafe { PAGE_TREE_ALLOCATOR.deallocate(buf + i * 4096) };
            }
            return Err(ErrorCode::InvalidProcessFile);
        }
    };

    let new_pid = match create_process(&context) {
        Ok(pid) => pid,
        Err(e) => {
            println!(level:error, "Failed to create process from file: {}, error: {:?}", path.to_string(), e);
            for i in 0..buf_pages {
                unsafe { PAGE_TREE_ALLOCATOR.deallocate(buf + i * 4096) };
            }
            return Err(e);
        }
    };
    for i in 0..buf_pages {
        unsafe { PAGE_TREE_ALLOCATOR.deallocate(buf + i * 4096) };
    }

    Ok(new_pid)
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
