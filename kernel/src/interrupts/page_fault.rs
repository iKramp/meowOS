use std::{mem_utils::VirtAddr, println, printlnc};

use crate::{acpi::cpu_locals::PageFaultHandleMode, interrupts::InterruptProcessorState, memory::paging::PageTree};

#[derive(Debug)]
#[allow(dead_code)] //not actually dead, is used in println
struct PageFaultErrorCode {
    protection_violation: bool,
    caused_by_write: bool,
    user_mode: bool,
    malformed_table: bool,
    instruction_fetch: bool,
}

impl From<u64> for PageFaultErrorCode {
    fn from(value: u64) -> Self {
        Self {
            protection_violation: value & (1 << 0) != 0,
            caused_by_write: value & (1 << 1) != 0,
            user_mode: value & (1 << 2) != 0,
            malformed_table: value & (1 << 3) != 0,
            instruction_fetch: value & (1 << 4) != 0,
        }
    }
}

pub extern "C" fn page_fault(proc_data: &mut InterruptProcessorState) {
    let locals = crate::acpi::cpu_locals::CpuLocals::get();
    let page_fault_mode = locals.page_fault_handle_mode;
    drop(locals);

    let _cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) _cr2);
    }

    if proc_data.interrupt_frame.rip >= core::ptr::addr_of!(crate::memory::probe_functions_start) as u64
        && proc_data.interrupt_frame.rip < core::ptr::addr_of!(crate::memory::probe_functions_end) as u64 + 0x1000
    {
        proc_data.interrupt_frame.rip = crate::memory::probe_fail as u64;
        return;
    }

    match page_fault_mode {
        PageFaultHandleMode::KernelPanic => fatal_page_fault(proc_data),
        PageFaultHandleMode::User => {
            todo!()
        }
    }
}

fn fatal_page_fault(proc_data: &InterruptProcessorState) -> ! {
    println!("{}", proc_data as *const InterruptProcessorState as usize);
    printlnc!(level:error,
        (0, 0, 255),
        "EXCEPTION: PAGE FAULT. error code: {:#X?}\nproc state: {:#X?}",
        PageFaultErrorCode::from(proc_data.err_code),
        proc_data
    );
    let mut page_tree = PageTree::new(PageTree::get_level4_addr());
    page_tree
        .get_page_table_entry_mut(VirtAddr(proc_data.interrupt_frame.rip & 0xFFFF_FFFF_FFFF_F000))
        .map(|entry| {
            println!(level:error,
                "Page fault at {:#X?} with entry: {:#X?}",
                proc_data.interrupt_frame.rip, entry
            );
        })
        .unwrap_or_else(|| {
            println!(level:error,"Page fault at {:#X?} with no entry", proc_data.interrupt_frame.rip);
        });
    unsafe {
        loop {
            core::arch::asm!("hlt");
        }
    }
}
