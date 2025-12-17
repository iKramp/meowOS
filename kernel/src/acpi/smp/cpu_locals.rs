use core::{mem::MaybeUninit, sync::atomic::{AtomicBool, AtomicU16, Ordering}};
use std::{
    boxed::Box,
    mem_utils::{VirtAddr, get_at_virtual_addr},
    sync::arc::Arc,
    sync::lock_info::{LockInfo, set_lock_info_func},
    vec::Vec,
};

use crate::{
    acpi::platform_info::PlatformInfo,
    interrupts::{self, idt::TablePointer},
    memory::stack::{KERNEL_STACK_SIZE_PAGES, prepare_kernel_stack},
    proc::ProcessData,
    task_runner::AsyncTaskData,
};

pub static mut CPU_LOCALS: MaybeUninit<Box<[VirtAddr]>> = MaybeUninit::uninit();

struct CpuLocalGetState {
    mut_borrow: AtomicBool,
    immut_borrow: AtomicU16,
}

#[repr(transparent)]
pub struct CpuLocalBinding {
    cpu_locals: &'static mut CpuLocals,
}

#[repr(transparent)]
pub struct CpuLocalBindingMut {
    cpu_locals: &'static mut CpuLocals,
}

impl Drop for CpuLocalBinding {
    fn drop(&mut self) {
        self.cpu_locals.get_state.immut_borrow.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Drop for CpuLocalBindingMut {
    fn drop(&mut self) {
        self.cpu_locals.get_state.mut_borrow.store(false, Ordering::Release);
    }
}

impl<'a> CpuLocalBinding {
    pub fn get(&'a self) -> &'a CpuLocals {
        self.cpu_locals
    }
}
impl<'a> CpuLocalBindingMut {
    pub fn get(&'a mut self) -> &'a mut CpuLocals {
        self.cpu_locals
    }
}

impl std::ops::Deref for CpuLocalBinding {
    type Target = CpuLocals;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl std::ops::DerefMut for CpuLocalBindingMut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get()
    }
}

impl std::ops::Deref for CpuLocalBindingMut {
    type Target = CpuLocals;
    fn deref(&self) -> &Self::Target {
        self.cpu_locals
    }
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
    pub apic_id: u8,
    pub processor_id: u8,
    pub int_depth: u32,
    pub proc_initialized: bool,
    pub atomic_context: bool,
    pub async_task_data: AsyncTaskData,
    pub lock_info: LockInfo,
    get_state: CpuLocalGetState,
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
        let bsp_local = get_at_virtual_addr::<CpuLocals>(old_bsp_local); //same addr
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
    let bsp_gdt = interrupts::create_new_gdt(bsp_stack_ptr);
    interrupts::load_gdt(bsp_gdt);
    let bsp_local = super::cpu_locals::CpuLocals::new(bsp_stack_ptr, KERNEL_STACK_SIZE_PAGES as u64, 0, 0, bsp_gdt);
    let bsp_local_ptr = add_cpu_locals(bsp_local);
    crate::msr::set_msr(0xC0000101, bsp_local_ptr.0);

    set_lock_info_func(|| unsafe { CpuLocals::get_lock_info() });
}

pub fn add_cpu_locals(locals: super::cpu_locals::CpuLocals) -> VirtAddr {
    unsafe {
        let apic_id = locals.apic_id;
        let cpu_locals = CPU_LOCALS.assume_init_mut();
        let ptr = std::Box::leak(std::Box::new(locals)) as *mut _ as *mut u64;
        ptr.write_volatile(ptr as u64); //write self pointer

        cpu_locals[apic_id as usize] = VirtAddr(ptr as u64);
        VirtAddr(cpu_locals[apic_id as usize].0)
    }
}

impl CpuLocals {
    pub fn new(kernel_stack_base: VirtAddr, stack_size_pages: u64, apic_id: u8, processor_id: u8, gdt_ptr: TablePointer) -> Self {
        Self {
            kernel_stack_base,
            stack_size_pages,
            apic_id,
            processor_id,
            gdt_ptr,
            current_process: None,
            async_task_data: AsyncTaskData::new(),
            proc_initialized: false,
            int_depth: 0,
            atomic_context: false,
            userspace_stack_base: 0,
            self_addr: VirtAddr(0), //will be set later
            lock_info: LockInfo::new(),
            get_state: CpuLocalGetState {
                mut_borrow: AtomicBool::new(false),
                immut_borrow: AtomicU16::new(0),
            },
        }
    }

    pub fn get() -> CpuLocalBinding {
        unsafe {
            let cpu_locals: *mut Self;
            core::arch::asm!(
                "mov {cpu_locals}, gs:0",
                cpu_locals = out(reg) cpu_locals
            );
            let immut_ref = &mut *cpu_locals;
            immut_ref.get_state.immut_borrow.fetch_add(1, Ordering::AcqRel);
            assert!(!immut_ref.get_state.mut_borrow.load(Ordering::Acquire), "CpuLocals already mutably borrowed");
            CpuLocalBinding { cpu_locals: immut_ref }
        }
    }

    pub fn get_mut() -> CpuLocalBindingMut {
        unsafe {
            let cpu_locals: *mut Self;
            core::arch::asm!(
                "mov {cpu_locals}, gs:0",
                cpu_locals = out(reg) cpu_locals
            );
            let mut_ref = &mut *cpu_locals;
            let prev_mut_borrow = mut_ref.get_state.mut_borrow.swap(true, Ordering::AcqRel);
            assert!(!prev_mut_borrow && mut_ref.get_state.immut_borrow.load(Ordering::Acquire) == 0, "CpuLocals already borrowed");
            CpuLocalBindingMut { cpu_locals: mut_ref }
        }
    }

    ///# Safety
    /// Ensure only 1 mutable reference at a time
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
