use crate::interrupts::InterruptProcessorState;
use crate::memory::VirtualMemoryRange;
use crate::memory::VirtualMemoryRangePermissions;
use crate::proc::PROCESS_ID_COUNTER;
use crate::proc::ProcessData;
use crate::proc::SCHEDULER;
use crate::proc::namespaces::MemoryNamespace;
use crate::proc::process_data::CpuStateType;
use std::error::ErrorCode;
use std::lock_w_info;
use std::string::ToString;
use std::sync::arc::Arc;
use std::{
    mem_utils::{VirtAddr, memset_physical_addr},
    println,
};

use crate::{
    memory,
    proc::{MappedMemoryRegion, MemoryContext, Pid},
};

use super::info::ContextInfo;

const DEFAULT_PROC_STACK_SIZE: usize = 0x1000; // 1kB initial stack

pub fn create_process(context_info: &ContextInfo) -> Result<Pid, ErrorCode> {
    println!("creating process with context: {:#?}", context_info);
    let pid = Pid(PROCESS_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed));
    let is_32_bit = context_info.is_32_bit();
    let cmdline = context_info.cmdline().to_string().into_boxed_str();
    let rip = context_info.entry_point().0;

    let memory_namespace = build_mem_context_for_new_proc(context_info)?;

    let stack = memory_namespace
        .regions()
        .iter()
        .find(|region| region)
        .expect("stack must exist for each proc");
    let rsp = stack.base.0 + (stack.size_pages as u64 * 0x1000) - 16; //-16 just in case (ret val and other things are 0)

    let cpu_state = InterruptProcessorState::new(rip, rsp);
    let process_data = ProcessData::new(
        pid,
        is_32_bit,
        cmdline,
        Arc::new(memory_namespace),
        CpuStateType::Interrupt(cpu_state),
    );

    let mut scheduler_lock = lock_w_info!(SCHEDULER);
    let scheduler = unsafe { scheduler_lock.assume_init_mut() };
    scheduler.accept_new_process(pid, process_data);

    Ok(pid)
}

pub fn build_initialized_memory_namespace(context: &ContextInfo, empty_namespace: &mut MemoryNamespace) {
    // map memory regions
    for region in context.mem_regions().iter() {
        let start = region.start().0;
        debug_assert!(start % 0x1000 == 0, "region start not page aligned");
        let end = start + region.size_pages() as u64 * 0x1000;
        for _page_addr in (start..end).step_by(0x1000) {
            // let _phys_addr_map = memory_tree.allocate_set_virtual(None, VirtAddr(page_addr));
            todo!("move to new memory api");
            // let page = memory_tree
            //     .get_page_table_entry_mut(VirtAddr(page_addr))
            //     .expect("page must exist after allocation");
            // page.set_writeable(region.flags().is_writeable());
            // page.set_user_accessible(true);
            // page.set_no_execute(!region.flags.is_executable());
        }
    }

    for mem_init in context.mem_init() {
        let first_page = mem_init.0.0 & (!0xfff);
        let last_page = (mem_init.0.0 + mem_init.1.len() as u64) & (!0xfff); //inclusive
        for _page_addr in (first_page..=last_page).step_by(0x1000) {
            todo!("move to new mem api");
            // let page = memory_tree
            //     .get_page_table_entry_mut(VirtAddr(page_addr))
            //     .expect("page must exist after allocation");
            // let physical_addr = page.address();
            // let physical_addr = PhysAddr(0);
            //
            // let start_mem_addr = page_addr.max(mem_init.0.0);
            // let start_data_index = (start_mem_addr - mem_init.0.0) as usize;
            // let mem_offset = start_mem_addr & 0xFFF;
            // let end_data_index = mem_init.1.len().min(start_data_index + 0x1000 - mem_offset as usize);
            //
            // unsafe {
            //     mem_utils::memcopy_physical_buffer(physical_addr + mem_offset, &mem_init.1[start_data_index..end_data_index])
            // }
        }
    }

    MemoryContext {
        initialized: true,
        is_32_bit: context.is_32_bit(),
        page_tree_root: memory_namespace,
        owned_memory_regions: context
            .mem_regions()
            .iter()
            .map(|region| MappedMemoryRegion {
                name: context.path().to_string().into_boxed_str(),
                base: VirtAddr(region.start().0),
                size_pages: region.size_pages() as u64,
            })
            .collect(),
    }
}

pub fn build_mem_context_for_new_proc(context: &ContextInfo) -> Result<MemoryNamespace, ErrorCode> {
    let mut mem_namespace = build_empty_memory_namespace();

    let current_root = memory::current_root();
    memory::set_cr3(mem_namespace.page_tree_root());

    build_initialized_memory_namespace(context, &mut mem_namespace);
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
    let mut mem_range = VirtualMemoryRange::create(memory::VirtualMemoryRangeCapacity::_1GB, permissions);
    mem_range.expand_to(stack_size_pages as u64);

    mem_namespace.add_mem_range(
        Arc::new(mem_range),
        "[stack]".to_string().into_boxed_str(),
        crate::proc::MemoryRangeType::Stack,
        free_area,
    )?;
    Ok(())
}

pub fn build_empty_memory_namespace() -> MemoryNamespace {
    let new_page_tree_root = memory::physical_allocator::allocate_frame();
    unsafe { memset_physical_addr(new_page_tree_root, 0x0, 0x1000) };

    let namespace = MemoryNamespace::new(new_page_tree_root);

    let existing_page_tree_root = memory::current_root();
    memory::copy_higher_half(existing_page_tree_root, new_page_tree_root);

    namespace
}

pub fn tear_down_mem_context(_context: &MemoryContext) {
    todo!("tear down mem context")
}
