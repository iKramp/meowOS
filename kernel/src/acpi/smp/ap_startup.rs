#![allow(clippy::erasing_op)]

#[link(name = "trampoline", kind = "static")]
unsafe extern "C" {
    pub fn ap_startup() -> !;
}

pub fn set_ap_initialized() {
    unsafe {
        let mut lock = false;
        while !lock {
            lock = super::CPU_LOCK.swap(true, core::sync::atomic::Ordering::Relaxed)
        }
        let cpus = super::CPUS_INITIALIZED.load(core::sync::atomic::Ordering::Relaxed);
        super::CPUS_INITIALIZED.store(cpus + 1, core::sync::atomic::Ordering::Relaxed);
    }
}

pub fn set_cpu_local(comm_lock: *mut u8) {
    let cpu_local_ptr = read_8_bytes(comm_lock);
    crate::msr::set_msr(0xC0000101, cpu_local_ptr);
}

pub fn read_8_bytes(comm_lock: *mut u8) -> u64 {
    read_4_bytes(comm_lock) as u64 | (read_4_bytes(comm_lock) as u64) << 32
}

pub fn read_4_bytes(comm_lock: *mut u8) -> u32 {
    read_2_bytes(comm_lock) as u32 | (read_2_bytes(comm_lock) as u32) << 16
}

pub fn read_2_bytes(comm_lock: *mut u8) -> u16 {
    (get_next_byte(comm_lock) as u16) | (get_next_byte(comm_lock) as u16) << 8
}

#[inline]
pub fn get_next_byte(comm_lock: *mut u8) -> u8 {
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
