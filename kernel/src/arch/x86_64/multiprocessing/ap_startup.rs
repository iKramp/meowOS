use core::{arch::asm, time::Duration};
use std::println;

use crate::{
    acpi::{
        cpu_locals::CpuLocals,
        smp::ap_startup::{read_8_bytes, set_ap_initialized, set_cpu_local},
    },
    arch::interrupts::{self, idt::IDT_POINTER},
    msr::{get_mtrr_cap, set_msr, set_mtrr_def_type},
};

#[unsafe(no_mangle)]
pub extern "C" fn ap_started_wait_loop() -> ! {
    let comm_lock: *mut u8;
    unsafe {
        core::arch::asm!(//pull the argument
            "mov {comm_lock}, rdi",
            comm_lock = out(reg) comm_lock
        );
    }

    set_mtrrs(comm_lock);
    set_cr_registers(comm_lock);

    set_cpu_local(comm_lock);
    load_cpu_locals();
    set_idt();
    let locals = CpuLocals::get();
    let processor_id = locals.processor_id;
    crate::acpi::init_acpi_ap(processor_id);

    set_ap_initialized();
    println!("AP {}: cpu woke up and received all data", processor_id);

    loop {
        unsafe {
            std::thread::sleep(Duration::from_millis(100));
            if crate::proc::PROC_INITIALIZED {
                //if proc initialized, we can start executing processes
                break;
            }
        }
    }

    crate::proc::init_ap();

    unsafe { core::arch::asm!("int 254", options(noreturn)) };
}

fn set_idt() {
    unsafe {
        asm!("lidt [{}]", "sti", in(reg) core::ptr::addr_of!(IDT_POINTER));
    }
}

fn load_cpu_locals() {
    let cpu_locals = CpuLocals::get();
    let cpu_local_ptr = cpu_locals.self_addr.0;
    let gdt_ptr = cpu_locals.arch_specific.gdt_ptr;
    interrupts::load_gdt(gdt_ptr);
    crate::msr::set_msr(0xC0000101, cpu_local_ptr);
}

fn set_cr_registers(comm_lock: *mut u8) {
    unsafe {
        let cr0 = read_8_bytes(comm_lock);
        let cr3 = read_8_bytes(comm_lock);
        let cr4 = read_8_bytes(comm_lock);

        core::arch::asm!(
            "mov cr0, {cr0}",
            "mov cr4, {cr4}",
            "mov cr3, {cr3}",
            cr0 = in(reg) cr0,
            cr3 = in(reg) cr3,
            cr4 = in(reg) cr4
        );
    }
}

fn set_mtrrs(comm_lock: *mut u8) {
    let mtrr_def = read_8_bytes(comm_lock);
    set_mtrr_def_type(mtrr_def);
    set_msr(0x250, read_8_bytes(comm_lock)); //fixed range
    set_msr(0x258, read_8_bytes(comm_lock));
    set_msr(0x259, read_8_bytes(comm_lock));
    set_msr(0x268, read_8_bytes(comm_lock));
    set_msr(0x269, read_8_bytes(comm_lock));
    set_msr(0x26A, read_8_bytes(comm_lock));
    set_msr(0x26B, read_8_bytes(comm_lock));
    set_msr(0x26C, read_8_bytes(comm_lock));
    set_msr(0x26D, read_8_bytes(comm_lock));
    set_msr(0x26E, read_8_bytes(comm_lock));
    set_msr(0x26F, read_8_bytes(comm_lock));

    let n = get_mtrr_cap() & 0xFF;

    for i in 0..n {
        set_msr(0x200 + (i as u32 * 2), read_8_bytes(comm_lock));
        set_msr(0x201 + (i as u32 * 2), read_8_bytes(comm_lock));
    }

    set_msr(0xC0000080, read_8_bytes(comm_lock));
}
