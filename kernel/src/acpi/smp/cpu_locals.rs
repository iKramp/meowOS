use core::mem::MaybeUninit;
use std::{
    boxed::Box,
    local_lock_read_w_info, local_lock_write_w_info, println,
    sync::{
        arc::Arc,
        local_lock::{LocalLock, LocalLockReadGuard, LocalLockWriteGuard},
        lock_info::{LockInfo, set_lock_info_func},
    },
    vec::Vec,
};

use crate::memory::addresses::*;

use crate::{
    acpi::{lapic_timer::AcceptedScheduledEvent, platform_info::PlatformInfo},
    interrupts::{self, idt::TablePointer},
    memory::stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack},
    proc::ProcessData,
    task_runner::AsyncTaskData,
};

pub static mut CPU_LOCALS: MaybeUninit<Box<[VirtAddr]>> = MaybeUninit::uninit();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultHandleMode {
    KernelPanic,
    User,
}

#[repr(C)]
pub struct CpuLocals {
    //keep this here
    pub self_addr: VirtAddr,
    //keep this here for syscall reasons
    pub kernel_stack_base: VirtAddr,
    //keep this here for syscall reasons
    pub userspace_stack_base: u64,
    pub stack_size_pages: u64,
    /// Points to TablePointer with base and limit of GDT
    pub gdt_ptr: TablePointer,

    pub current_process: Option<Arc<ProcessData>>,
    //id of scheduled event for preemtion, used to cancel preemption
    pub preemtion_id: Option<u64>,

    pub apic_id: u8,
    pub processor_id: u8,
    pub int_depth: u32,
    pub proc_initialized: bool,
    pub atomic_context: bool,
    pub async_task_data: AsyncTaskData,
    pub lock_info: LockInfo,
    pub page_fault_handle_mode: PageFaultHandleMode,
    lock_addr: VirtAddr,

    pub scheduled_event_id_counter: u64,
    pub scheduled_events: Vec<AcceptedScheduledEvent>,
}

pub fn init(platform_info: &PlatformInfo) {
    let num_cpus = platform_info.max_apic_id as usize + 1;
    #[allow(clippy::slow_vector_initialization)] //it's non const ffs
    let mut vec = Vec::with_capacity(num_cpus);
    vec.resize(num_cpus, VirtAddr(0));
    let mut locals_boxed_slice = MaybeUninit::new(vec.into_boxed_slice());
    unsafe { std::mem::swap(&mut locals_boxed_slice, &mut CPU_LOCALS) }
    let old_bsp_local = unsafe { locals_boxed_slice.assume_init_mut()[0] };

    unsafe {
        let cpu_locals = CPU_LOCALS.assume_init_mut();
        let apic_id = platform_info.boot_processor.apic_id;
        cpu_locals[apic_id as usize] = old_bsp_local;
        let bsp_local = get_at_addr::<CpuLocals, _>(old_bsp_local); //same addr
        bsp_local.apic_id = apic_id;
        bsp_local.processor_id = platform_info.boot_processor.processor_id;
        let bsp_local_ptr_addr = VirtAddr(cpu_locals[apic_id as usize].0);
        crate::msr::set_msr(0xC0000101, bsp_local_ptr_addr.0);
    }

    //explicitly drop so compiler doesn't drop before writing to msr
    #[allow(clippy::drop_non_drop)]
    drop(locals_boxed_slice);
}

pub fn init_dummy_cpu_locals() {
    #[allow(clippy::slow_vector_initialization)] //it's non const ffs
    let mut vec = Vec::with_capacity(1);
    vec.resize(1, VirtAddr(0));
    unsafe { CPU_LOCALS = MaybeUninit::new(vec.into_boxed_slice()) }

    let bsp_stack_ptr = prepare_kernel_stack(KERNEL_STACK_SIZE_PAGES);
    println!(level:info, "BSP stack ptr: {:016X}, size: {:X}", bsp_stack_ptr.0, KERNEL_STACK_SIZE_PAGES as u64 * 4096);
    let bsp_gdt = interrupts::create_new_gdt(bsp_stack_ptr);
    interrupts::load_gdt(bsp_gdt);
    let bsp_local = CpuLocals::new(bsp_stack_ptr, KERNEL_STACK_SIZE_PAGES as u64, 0, 0, bsp_gdt);
    let bsp_local_ptr = add_cpu_locals(bsp_local);
    crate::msr::set_msr(0xC0000101, bsp_local_ptr.0);

    set_lock_info_func(|| unsafe { CpuLocals::get_lock_info() });
}

pub fn add_cpu_locals(locals: CpuLocals) -> VirtAddr {
    let apic_id = locals.apic_id;

    let mut locals_on_heap = Box::new(locals);
    let locals_addr = locals_on_heap.as_ref() as *const CpuLocals as u64;
    locals_on_heap.self_addr = VirtAddr(locals_addr);

    let locked_locals = Box::new(LocalLock::new(locals_on_heap));
    let lock_addr = Box::leak(locked_locals) as *const _ as u64;

    unsafe {
        let cpu_locals_arr = CPU_LOCALS.assume_init_mut();

        let locals_ptr = locals_addr as *mut CpuLocals;
        let lock_addr_ptr = (locals_ptr.byte_add(core::mem::offset_of!(CpuLocals, lock_addr))) as *mut VirtAddr;
        lock_addr_ptr.write(VirtAddr(lock_addr));

        cpu_locals_arr[apic_id as usize] = VirtAddr(locals_addr);
        VirtAddr(cpu_locals_arr[apic_id as usize].0)
    }
}

impl CpuLocals {
    pub fn new(kernel_stack_base: VirtAddr, stack_size_pages: u64, apic_id: u8, processor_id: u8, gdt_ptr: TablePointer) -> Self {
        Self {
            self_addr: VirtAddr(0), //will be set later
            kernel_stack_base,
            userspace_stack_base: 0,
            stack_size_pages,
            gdt_ptr,

            current_process: None,
            preemtion_id: None,

            apic_id,
            processor_id,
            int_depth: 0,
            proc_initialized: false,
            atomic_context: false,
            async_task_data: AsyncTaskData::new(),
            lock_info: LockInfo::new(),
            page_fault_handle_mode: PageFaultHandleMode::KernelPanic,
            lock_addr: VirtAddr(0),
            scheduled_event_id_counter: 0,
            scheduled_events: Vec::new(),
        }
    }

    pub fn get() -> LocalLockReadGuard<'static, Box<CpuLocals>> {
        unsafe {
            let cpu_locals: *mut Self;
            core::arch::asm!(
                "mov {cpu_locals}, gs:0",
                cpu_locals = out(reg) cpu_locals
            );
            let immut_ref = &mut *cpu_locals;

            let lock_addr = immut_ref.lock_addr;
            let lock = get_at_addr::<LocalLock<Box<CpuLocals>>, _>(lock_addr);
            local_lock_read_w_info!(lock)
        }
    }

    pub fn get_mut() -> LocalLockWriteGuard<'static, Box<CpuLocals>> {
        unsafe {
            let cpu_locals: *mut Self;
            core::arch::asm!(
                "mov {cpu_locals}, gs:0",
                cpu_locals = out(reg) cpu_locals
            );
            let mut_ref = &mut *cpu_locals;

            let lock_addr = mut_ref.lock_addr;
            let lock = get_at_addr::<LocalLock<Box<CpuLocals>>, _>(lock_addr);
            local_lock_write_w_info!(lock)
        }
    }

    ///# Safety
    /// Ensure only 1 mutable reference at a time
    /// NEVER get a lock when calling this, because this is called when Cpu locals lock is being
    /// acquired. Interrupts are already disabled when get_lock_info is called
    pub unsafe fn get_lock_info() -> &'static mut LockInfo {
        unsafe {
            let cpu_locals: *mut Self;
            core::arch::asm!(
                "mov {cpu_locals}, gs:0",
                cpu_locals = out(reg) cpu_locals
            );
            let mut_ref = &mut *cpu_locals;
            &mut mut_ref.lock_info
        }
    }
}

//FS register contains thread local storage of a process
