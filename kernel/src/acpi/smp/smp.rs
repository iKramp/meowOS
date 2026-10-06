use crate::{
    arch::multiprocessing::{init_single_cpu, set_trampoline_values},
    memory::addresses::*,
    println,
};
use core::sync::atomic::{AtomicBool, AtomicU8};

pub static mut CPU_LOCK: AtomicBool = AtomicBool::new(false);
pub static mut CPUS_INITIALIZED: AtomicU8 = AtomicU8::new(0);

//custom data starts at 0x4 from ap_startup

use crate::acpi::platform_info::PlatformInfo;

pub fn wake_cpus(platform_info: &PlatformInfo) {
    copy_trampoline();

    unsafe {
        let destination = VirtAddr::from(crate::memory::TRAMPOLINE_RESERVED.0).0 as *mut u8;
        for cpu in platform_info.application_processors.iter().enumerate() {
            init_single_cpu(destination, cpu);
        }
    }
}

fn copy_trampoline() {
    let destination_phys = unsafe { crate::memory::TRAMPOLINE_RESERVED.0 };
    println!("copying trampoline to {:x?}", destination_phys);

    assert!(
        destination_phys.0 <= 0xFFFFF,
        "memory addresss should be less than 1MB to initialize APs"
    );

    let source = super::ap_startup::ap_startup as *const () as *const u8;
    let destination_virt = VirtAddr::from(destination_phys).0 as *mut u8;
    for i in 0..0x1000 {
        unsafe {
            destination_virt.add(i).write_volatile(source.add(i).read_volatile());
        }
    }

    set_trampoline_values(destination_virt, destination_phys);
}

pub fn wait_for_cpus(num_cpus: u8) {
    loop {
        let cpus_initialized = unsafe { CPUS_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed) };
        if cpus_initialized == num_cpus {
            break;
        }
    }
}

pub fn send_cpu_locals(ptr: u64, comm_lock: *mut u8) {
    send_u64(ptr, comm_lock);
}

pub fn send_u64(data: u64, comm_lock: *mut u8) {
    send_bytes(&data.to_ne_bytes(), comm_lock);
}

pub fn send_u16(data: u16, comm_lock: *mut u8) {
    send_bytes(&data.to_ne_bytes(), comm_lock);
}

fn send_bytes(data: &[u8], comm_lock: *mut u8) {
    for byte in data {
        send_byte(*byte, comm_lock);
    }
}

fn send_byte(data_byte: u8, comm_lock: *mut u8) {
    unsafe {
        let mut byte;
        loop {
            byte = 1_u8;
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
                "mov {byte}, [{comm_lock}]",
                byte = out(reg_byte) data_ready,
                comm_lock = in(reg) comm_lock.add(1),
            );
            if data_ready == 1 {
                //ap didn't read yet
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
        core::arch::asm!(//write data
            "mov [{comm_lock}], {data}",
            "clflush [{comm_lock}]",
            data = in(reg_byte) data_byte,
            comm_lock = in(reg) comm_lock.add(2),
        );
        core::arch::asm!(//set pending data
            "mov [{comm_lock}], {one}",
            "clflush [{comm_lock}]",
            one = in(reg_byte) 1_u8,
            comm_lock = in(reg) comm_lock.add(1),
        );
        core::arch::asm!(//release lock
            "mov [{comm_lock}], {zero}",
            "clflush [{comm_lock}]",
            comm_lock = in(reg) comm_lock,
            zero = in(reg_byte) 0_u8,
        );
    }
}
