use crate::interrupts::InterruptProcessorState;
use crate::memory::VirtualMemoryRange;
use crate::memory::VirtualMemoryRangeGrowDirection;
use crate::memory::VirtualMemoryRangePermissions;
use crate::memory::VirtualMemoryRangeType;
use crate::memory::current_root;
use crate::proc::MemoryRangeType;
use crate::proc::PROCESS_ID_COUNTER;
use crate::proc::ProcNamespaces;
use crate::proc::ProcessData;
use crate::proc::SCHEDULER;
use crate::proc::SyscallNamespace;
use crate::proc::namespaces::MemoryNamespace;
use crate::proc::process_data::CpuStateType;
use std::error::ErrorCode;
use std::lock_w_info;
use std::string::ToString;
use std::sync::arc::Arc;
use std::{mem_utils::VirtAddr, println};

use crate::{memory, proc::Pid};

use super::info::ContextInfo;

const DEFAULT_PROC_STACK_SIZE: usize = 0x1000; // 1kB initial stack

pub fn create_process_from_context(context_info: &ContextInfo) -> Result<Pid, ErrorCode> {
    println!("creating process with context: {:#?}", context_info);
    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let is_32_bit = context_info.is_32_bit();
    let cmdline = context_info.cmdline().to_string().into_boxed_str();
    let rip = context_info.entry_point().0;

    let memory_namespace = build_mem_namespace_for_new_proc(context_info)?;
    let dynamic_namespace_data = lock_w_info!(memory_namespace.dynamic_data);

    let stack = dynamic_namespace_data
        .regions()
        .iter()
        .find(|region| region.range_type == MemoryRangeType::Stack)
        .expect("stack must exist for each proc");
    let rsp = stack.shared_range.reserved_range(stack.map_address).end.0 - 16;
    drop(dynamic_namespace_data);

    let memory_namespace = Arc::new(memory_namespace);
    let syscall_namespace = Arc::new(SyscallNamespace::default(crate::proc::get_namespace_id()));

    let cpu_state = InterruptProcessorState::new(rip, rsp);
    let process_data = ProcessData::new(
        pid,
        is_32_bit,
        cmdline,
        CpuStateType::Interrupt(cpu_state),
        ProcNamespaces::new(memory_namespace, syscall_namespace),
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);

    Ok(pid)
}
pub fn build_initialized_memory_namespace(
    context: &ContextInfo,
    mem_namespace: MemoryNamespace,
) -> Result<MemoryNamespace, ErrorCode> {
    let proc_mem_range = VirtualMemoryRange::create(
        memory::VirtualMemoryRangeCapacity::_05TB,
        VirtualMemoryRangePermissions(0),
        VirtualMemoryRangeType::Manual,
    );
    let proc_mem_range_bounds = proc_mem_range.reserved_range(VirtAddr(0));

    // map memory regions
    for region in context.mem_regions().iter() {
        let start = region.start();
        debug_assert!(start.0 % 0x1000 == 0, "region start not page aligned");
        let end = start + region.size_pages() as u64 * 0x1000;
        if end > proc_mem_range_bounds.end {
            println!(
                "region end {:#x?} exceeds process memory range bounds {:#x?}",
                end, proc_mem_range_bounds.end.0
            );
            return Err(ErrorCode::InvalidProcessFile);
        }

        let mut perms = VirtualMemoryRangePermissions(0); //allow writing
        perms.set_write(true);

        let page_start = start.0 / 0x1000;
        let page_end = end.0 / 0x1000;

        proc_mem_range.allocate_manual(page_start as u32..page_end as u32, perms)?;
    }

    let mem_range = Arc::new(proc_mem_range);

    mem_namespace
        .add_mem_range(
            mem_range.clone(),
            context.path().to_string().into_boxed_str(),
            MemoryRangeType::Code,
            VirtAddr(0),
        )
        .expect("mapping process memory failed");

    for mem_init in context.mem_init() {
        let dest_ptr = mem_init.0.0 as *mut u8;
        let src_ptr = mem_init.1.as_ptr();
        let len = mem_init.1.len();
        unsafe {
            core::ptr::copy_nonoverlapping(src_ptr, dest_ptr, len);
        }
    }

    //set prot
    for region in context.mem_regions().iter() {
        let start = region.start();
        let end = start + region.size_pages() as u64 * 0x1000;

        let mut perms = VirtualMemoryRangePermissions(0);
        perms.set_write(region.flags().is_writeable());
        perms.set_execute(region.flags().is_executable());

        memory::set_prot(current_root(), start..end, perms, 4, VirtAddr(0));
    }

    Ok(mem_namespace)
}

pub fn build_mem_namespace_for_new_proc(context: &ContextInfo) -> Result<MemoryNamespace, ErrorCode> {
    let mem_namespace = build_empty_memory_namespace();

    let current_root = memory::current_root();
    memory::set_cr3(mem_namespace.page_tree_root());

    let mut mem_namespace = build_initialized_memory_namespace(context, mem_namespace)?;
    let stack_size_pages = DEFAULT_PROC_STACK_SIZE.div_ceil(0x1000) as u8; // convert to pages

    //add stack
    if let Err(e) = add_stack(&mut mem_namespace, stack_size_pages) {
        memory::set_cr3(current_root);
        return Err(e);
    }
    memory::set_cr3(current_root);
    Ok(mem_namespace)
}

pub fn add_stack(mem_namespace: &mut MemoryNamespace, stack_size_pages: u8) -> Result<(), ErrorCode> {
    let Some(free_area) = mem_namespace.find_hole(memory::VirtualMemoryRangeCapacity::_1GB) else {
        println!("no free area for stack");
        return Err(ErrorCode::OutOfMemory);
    };

    let mut permissions = VirtualMemoryRangePermissions(0);
    permissions.set_write(true);
    permissions.set_execute(false);
    let mem_range = VirtualMemoryRange::create(
        memory::VirtualMemoryRangeCapacity::_1GB,
        permissions,
        VirtualMemoryRangeType::Managed(VirtualMemoryRangeGrowDirection::Down),
    );
    mem_range.expand_to(stack_size_pages as u32).expect("adding stack failed");
    let stack_highest_addr = mem_range.reserved_range(free_area).end;

    mem_namespace.add_mem_range(
        Arc::new(mem_range),
        "[stack]".to_string().into_boxed_str(),
        crate::proc::MemoryRangeType::Stack,
        free_area,
    )?;

    unsafe { *((stack_highest_addr.0 - 0x08) as *mut u64) = 0 };
    unsafe { *((stack_highest_addr.0 - 0x10) as *mut u64) = 0 };
    println!("added stack mem range");
    Ok(())
}

pub fn build_empty_memory_namespace() -> MemoryNamespace {
    let namespace = MemoryNamespace::new(crate::proc::get_namespace_id());

    let new_page_tree_root = namespace.page_tree_root();
    let existing_page_tree_root = memory::current_root();
    memory::copy_higher_half(existing_page_tree_root, new_page_tree_root);

    namespace
}
