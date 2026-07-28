use crate::memory::{addresses::*, kernel_map, physical_allocator};
use core::mem::MaybeUninit;
use std::println;

use bitfield::bitfield;
use reg_map::RegMap;

use crate::{
    acpi,
    memory::{self, LiminePat},
};

use super::Timer;

pub(super) struct HpetWrapper {
    registers: MaybeUninit<HpetRegistersPtr<'static>>,
    started: std::time::Instant,
    is_64_bit: bool,
    allocated_page: OwnedVirtAddr,
    ticks_on_start: u64,
}

impl HpetWrapper {
    pub const fn new() -> Self {
        Self {
            registers: MaybeUninit::uninit(),
            started: std::time::UNIX_EPOCH,
            is_64_bit: false,
            allocated_page: OwnedVirtAddr(VirtAddr(0)),
            ticks_on_start: 0,
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

        self.registers = MaybeUninit::new(registers);
        self.is_64_bit = general_cap.count_size_cap();
        true
    }

    fn start_timer(&mut self) -> bool {
        let self_regs = unsafe { self.registers.assume_init_ref() };

        let mut gen_conf = self_regs.general_configuration().read();
        gen_conf.set_enabled(true);
        self_regs.general_configuration().write(gen_conf);
        true
    }

    // let self_regs = unsafe { self.registers.assume_init_ref() };
    // let timer_conf = self_regs.timer_0();
    // timer_conf.cmp_value().write(self.cmp_value);
    // let mut conf_reg = timer_conf.conf_and_cap().read();
    // if !conf_reg.periodic_capable() {
    //     return false;
    // }
    // conf_reg.set_int_type(false); //edge triggered
    // conf_reg.set_int_enable(true); //enable interrupts
    // conf_reg.set_type(true); //periodic
    // const IO_APIC_ROUTE: u8 = TIMER_INTERRUPT_VECTOR as u8 - 32;
    // conf_reg.set_int_route(IO_APIC_ROUTE as u64); //route to IO APIC
    // timer_conf.conf_and_cap().write(conf_reg);
    //
    // let mut gen_conf = self_regs.general_configuration().read();
    // gen_conf.set_enabled(true);
    // self_regs.general_configuration().write(gen_conf);
    // true

    fn get_main_counter(&self) -> u64 {
        let self_regs = unsafe { self.registers.assume_init_ref() };
        if self.is_64_bit {
            self_regs.main_counter_value().read()
        } else {
            self_regs.main_counter_value().read() & 0xFFFFFFFF
        }
    }
}

impl Drop for HpetWrapper {
    fn drop(&mut self) {
        if self.allocated_page.0 == VirtAddr(0) {
            //make self.allocated_page be a legit droppable page
            let mut tmp = kernel_map(physical_allocator::allocate());
            core::mem::swap(&mut tmp, &mut self.allocated_page);
            core::mem::forget(tmp); //not efficient but won't be called much
            return;
        }

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

        println!("Hpet registers before enabling: {:#?}", unsafe {
            self.registers.assume_init_ref().as_ptr().read()
        });

        self.start_timer()
    }

    fn get_time(&self) -> std::time::Instant {
        let main_count = self.get_main_counter();
        let elapsed_ticks = main_count.wrapping_sub(self.ticks_on_start);
        let period_femptosecs = unsafe {
            self.registers
                .assume_init_ref()
                .general_capabilities()
                .read()
                .counter_clk_period()
        };
        let elapsed_femtoseconds = elapsed_ticks as u128 * period_femptosecs as u128;
        let elapsed_nanos = elapsed_femtoseconds / 1_000_000; // convert femtoseconds to nanoseconds
        let elapsed_nanos = elapsed_nanos as u64; // ensure it's a u64 for Duration
        let elapsed_duration = core::time::Duration::new(elapsed_nanos / 1_000_000_000, (elapsed_nanos % 1_000_000_000) as u32);

        self.started + elapsed_duration
    }

    fn calibrate(&mut self, now: std::time::Instant) {
        let ticks_now = self.get_main_counter();
        self.ticks_on_start = ticks_now;
        self.started = now;
    }
}

#[derive(RegMap, Debug)]
#[repr(C)]
struct HpetRegisters {
    general_capabilities: GeneralCap,
    res_0: u64,
    general_configuration: GeneralConfig,
    res_1: u64,
    interrupt_status: IntStatus,
    res_2: [u64; 25],
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
    impl Debug;
    enabled, set_enabled: 0;
    ///legacy routing to IRQ2 in IO APIC. Don't do this
    leg_rt, set_leg_rt: 1;
}

bitfield! {
    #[derive(RegMap)]
    struct IntStatus(u64);
    impl Debug;
    timer_0, clear_timer_0: 0;
    timer_1, clear_timer_1: 1;
    timer_2, clear_timer_2: 2;
}

#[derive(RegMap, Debug)]
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
