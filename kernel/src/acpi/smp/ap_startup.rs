#![allow(clippy::erasing_op)]

use crate::interrupts::{self, idt::IDT_POINTER};
use crate::println;
use core::arch::asm;
use core::time::Duration;

use crate::msr::{get_mtrr_cap, set_msr, set_mtrr_def_type};

use super::cpu_locals;

#[link(name = "trampoline", kind = "static")]
unsafe extern "C" {
    pub fn ap_startup() -> !;
}

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
    set_idt();
    let locals = super::cpu_locals::CpuLocals::get();
    let processor_id = locals.processor_id;
    crate::acpi::init_acpi_ap(processor_id);

    set_initialized();
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

fn set_initialized() {
    unsafe {
        let mut lock = false;
        while !lock {
            lock = super::CPU_LOCK.swap(true, core::sync::atomic::Ordering::Relaxed)
        }
        let cpus = super::CPUS_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
        super::CPUS_INITIALIZED.store(cpus + 1, core::sync::atomic::Ordering::Relaxed);
    }
}

fn set_cpu_local(comm_lock: *mut u8) {
    let cpu_local_ptr = read_8_bytes(comm_lock);
    crate::msr::set_msr(0xC0000101, cpu_local_ptr);
    let cpu_locals = cpu_locals::CpuLocals::get();
    let gdt_ptr = cpu_locals.gdt_ptr;
    interrupts::load_gdt(gdt_ptr);

    //set again because setting gdt clears gs register
    crate::msr::set_msr(0xC0000101, cpu_local_ptr);
}

fn set_idt() {
    unsafe {
        asm!("lidt [{}]", "sti", in(reg) core::ptr::addr_of!(IDT_POINTER));
    }
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

fn read_8_bytes(comm_lock: *mut u8) -> u64 {
    read_4_bytes(comm_lock) as u64 | (read_4_bytes(comm_lock) as u64) << 32
}

fn read_4_bytes(comm_lock: *mut u8) -> u32 {
    read_2_bytes(comm_lock) as u32 | (read_2_bytes(comm_lock) as u32) << 16
}

fn read_2_bytes(comm_lock: *mut u8) -> u16 {
    (get_next_byte(comm_lock) as u16) | (get_next_byte(comm_lock) as u16) << 8
}

#[inline]
fn get_next_byte(comm_lock: *mut u8) -> u8 {
    unsafe {
        let mut byte;
        loop {
            byte = 1;
            core::arch::asm!(//obtain comm lock
                "xchg {byte}, [{comm_lock}]",
                byte = inout(reg_byte) byte,
                comm_lock = in(reg) comm_lock,
            );
            if byte != 0 {
                continue;
            }
            let data_ready: u8;
            core::arch::asm!(//check if there's pending data
                "mov {data_ready}, [{comm_lock}]",
                data_ready = out(reg_byte) data_ready,
                comm_lock = in(reg) comm_lock.add(1),
            );
            if data_ready == 0 {
                //bsp didn't write yet
                core::arch::asm!(//release lock
                    "mov [{comm_lock}], {zero}",
                    "clflush [{comm_lock}]",
                    comm_lock = in(reg) comm_lock,
                    zero = in(reg_byte) 0_u8,
                );
                continue;
            } else {
                break;
            }
        }
        core::arch::asm!(//read data
            "mov {data}, [{comm_lock}]",
            data = out(reg_byte) byte,
            comm_lock = in(reg) comm_lock.add(2),
        );
        core::arch::asm!(//unset pending data
            "mov [{comm_lock}], {zero}",
            "clflush [{comm_lock}]",
            zero = in(reg_byte) 0_u8,
            comm_lock = in(reg) comm_lock.add(1),
        );
        core::arch::asm!(//release lock
            "mov [{comm_lock}], {zero}",
            "clflush [{comm_lock}]",
            comm_lock = in(reg) comm_lock,
            zero = in(reg_byte) 0_u8,
        );
        byte
    }
}
