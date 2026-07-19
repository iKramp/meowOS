use crate::memory::addresses::*;
use core::{mem::MaybeUninit, sync::atomic::AtomicU64};
use std::printlnc;

use bitfield::bitfield;
use reg_map::RegMap;

use crate::{
    acpi,
    clocks::TIMER_INTERRUPT_VECTOR,
    memory::{self, LiminePat},
};

use super::Timer;

pub(super) struct HpetWrapper {
    registers: MaybeUninit<HpetRegistersPtr<'static>>,
    started: std::time::Instant,
    is_64_bit: bool,
    cmp_value: u64,
    seconds_since: AtomicU64,
    last_main_count: AtomicU64,
    allocated_page: OwnedVirtAddr,
}

impl HpetWrapper {
    pub const fn new() -> Self {
        Self {
            registers: MaybeUninit::uninit(),
            started: std::time::UNIX_EPOCH,
            is_64_bit: false,
            cmp_value: 0,
            seconds_since: AtomicU64::new(0),
            last_main_count: AtomicU64::new(0),
            allocated_page: OwnedVirtAddr(VirtAddr(0)),
        }
    }

    fn get_registers(&mut self, reg_phys_addr: PhysAddr) -> bool {
        let owned_addr = OwnedPhysAddr(reg_phys_addr);

        let (virt_range, entry) = unsafe { memory::kernel_manual_map(owned_addr.into(), None) };
        let mut owned_virt_addr = virt_range.into_owned_virt_addr();
        let virt_addr = owned_virt_addr.0;

        entry.set_pat(LiminePat::UC, virt_addr);

        core::mem::swap(&mut self.allocated_page, &mut owned_virt_addr);
        core::mem::forget(owned_virt_addr); //uninitialized

        let registers = unsafe { HpetRegistersPtr::from_ptr(virt_addr.0 as *mut HpetRegisters) };
        let general_cap = registers.general_capabilities().read();
        let period_femptosecs = general_cap.counter_clk_period();
        let counter_size_bits = 32 * (1 + general_cap.count_size_cap() as u64);
        let counter_size = 2_u64.pow(counter_size_bits as u32 - 1);
        let mult = period_femptosecs.checked_mul(counter_size);
        let is_ok = if let Some(mult) = mult {
            mult > 10_u64.pow(15) // 1 second in femtoseconds
        } else {
            //overflow, timer is more than capable of 1 second intervals
            true
        };
        if !is_ok {
            printlnc!((255, 0, 0), "HPET: not capable of 1 second intervals");
            return false;
        }

        let periods_in_second = 10_u64.pow(15) / period_femptosecs;

        self.registers = MaybeUninit::new(registers);
        self.is_64_bit = general_cap.count_size_cap();
        self.cmp_value = periods_in_second;
        true
    }

    fn start_timer(&mut self) -> bool {
        let self_regs = unsafe { self.registers.assume_init_ref() };
        let timer_conf = self_regs.timer_0();
        timer_conf.cmp_value().write(self.cmp_value);
        let mut conf_reg = timer_conf.conf_and_cap().read();
        if !conf_reg.periodic_capable() {
            return false;
        }
        conf_reg.set_int_type(false); //edge triggered
        conf_reg.set_int_enable(true); //enable interrupts
        conf_reg.set_type(true); //periodic
        const IO_APIC_ROUTE: u8 = TIMER_INTERRUPT_VECTOR as u8 - 32;
        conf_reg.set_int_route(IO_APIC_ROUTE as u64); //route to IO APIC
        timer_conf.conf_and_cap().write(conf_reg);

        let mut gen_conf = self_regs.general_configuration().read();
        gen_conf.set_enabled(true);
        self_regs.general_configuration().write(gen_conf);
        true
    }
}

impl Drop for HpetWrapper {
    fn drop(&mut self) {
        let self_regs = unsafe { self.registers.assume_init_ref() };

        //disable timer
        let mut gen_conf = self_regs.general_configuration().read();
        gen_conf.set_enabled(false);
        self_regs.general_configuration().write(gen_conf);
    }
}

impl Timer for HpetWrapper {
    fn init(&mut self) -> bool {
        let hpet_table;
        unsafe {
            let Some(hpet_table_phys_addr) = acpi::ACPI_TABLE_MAP.get("HPET") else {
                return false;
            };
            hpet_table = get_at_addr::<acpi::HpetTable, _>(*hpet_table_phys_addr);
        }
        let hpet_regs = hpet_table.get_addr();
        if !self.get_registers(hpet_regs) {
            return false;
        }
        self.start_timer()
    }

    fn get_time(&self) -> std::time::Instant {
        let self_regs = unsafe { self.registers.assume_init_ref() };
        let last_main_count = self.last_main_count.load(core::sync::atomic::Ordering::Relaxed);

        let prev_seconds = 0;
        let mut new_seconds = self.seconds_since.load(core::sync::atomic::Ordering::SeqCst);
        let mut main_cnt = 0;
        while prev_seconds != new_seconds {
            main_cnt = if self.is_64_bit {
                self_regs.main_counter_value().read()
            } else {
                self_regs.main_counter_value().read() & 0xFFFFFFFF
            };
            new_seconds = self.seconds_since.load(core::sync::atomic::Ordering::SeqCst);
        }

        let timer_0_cnt = (main_cnt.wrapping_sub(last_main_count)) % self.cmp_value;
        let nanos = (timer_0_cnt * 1_000_000_000) / self.cmp_value;
        std::time::Instant::from_duration_since_epoch(core::time::Duration::new(new_seconds, nanos as u32))
    }

    fn calibrate(&mut self, now: std::time::Instant) {
        self.started = now;
    }

    fn service_interrupt(&self) {
        self.seconds_since.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        self.last_main_count
            .fetch_add(self.cmp_value, core::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(RegMap)]
#[repr(C)]
struct HpetRegisters {
    general_capabilities: GeneralCap,
    res_0: u64,
    general_configuration: GeneralConfig,
    res_1: u64,
    interrupt_status: IntStatus,
    res_2: u64,
    ///only write when timer is halted
    ///reads will return the current count value
    main_counter_value: u64,
    res_3: u64,
    timer_0: TimerConfig,
    timer_1: TimerConfig,
    timer_2: TimerConfig,
}

bitfield! {
    #[derive(RegMap)]
    struct GeneralCap(u64);
    impl Debug;
    rev_id, _: 7, 0;
    num_tim_cap, _: 12, 8;
    count_size_cap, _: 13;
    leg_route_cap, _: 15;
    vnedor_id, _: 31, 16;
    counter_clk_period, _: 63, 32;
}

bitfield! {
    #[derive(RegMap)]
    struct GeneralConfig(u64);
    enabled, set_enabled: 0;
    ///legacy routing to IRQ2 in IO APIC. Don't do this
    leg_rt, set_leg_rt: 1;
}

bitfield! {
    #[derive(RegMap)]
    struct IntStatus(u64);
    timer_0, clear_timer_0: 0;
    timer_1, clear_timer_1: 1;
    timer_2, clear_timer_2: 2;
}

#[derive(RegMap)]
#[repr(C)]
struct TimerConfig {
    conf_and_cap: TimerConfAndCap,
    cmp_value: u64,
    fsb_int_route: u64,
    res: u64,
}

bitfield! {
    #[derive(RegMap)]
    struct TimerConfAndCap(u64);
    impl Debug;
    ///0: edge tiggered
    ///1: level triggered
    int_type, set_int_type: 1;
    ///only controls interrupt, not operation of the timer
    int_enable, set_int_enable: 2;
    ///1: one-shot
    ///2: periodic
    _type, set_type: 3;
    periodic_capable, _: 4;
    ///0: 32 bits
    ///1: 64 bits
    size, _: 5;
    ///Set in periodic mode BEFORE setting the value of the timer
    _, velue_set: 6;
    _32_bit, set_32_bit: 8;
    ///route in the IO APIC
    int_route, set_int_route: 13, 9;
    //more useless fields
}
