#![allow(non_snake_case)]
#![allow(clippy::identity_op)]

use crate::memory::addresses::*;
use core::{array, fmt::Debug, mem::MaybeUninit, ops::DerefMut, sync::atomic::AtomicU32, task::Waker, time::Duration};
use std::{
    boxed::Box,
    error::KernelError,
    ffi_future, kerror, kerror_unwrapped, lock_w_info, println,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock, rw_lock::RWSpinlock},
    vec::Vec,
    w_lock_w_info,
};

use bitfield::bitfield;
use reg_map::RegMap;

use crate::{
    drivers::{
        block_device::{
            ahci::fis::{D2HRegisterFis, IdentifyStructure, PioSetupFis},
            disk::BlockDevice,
        },
        pci::{self, LegacyPciDriver},
    },
    memory::{self, physical_allocator},
    task_runner::{self, block_task},
};

use super::fis::{FisType, H2DRegFisPmport, H2DRegisterFis};

//we assume 48 bit lba
const READ_DMA: u8 = 0x25;
const WRITE_DMA: u8 = 0x35;

static OPERATIONS: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub struct AhciDriver {
    controller: MaybeUninit<AhciController>,
}

impl AhciDriver {
    pub(super) fn new() -> Self {
        Self {
            controller: MaybeUninit::uninit(),
        }
    }
}

impl LegacyPciDriver for AhciDriver {
    fn init(&mut self, dev: &pci::LegacyPciDevice) -> Result<(), KernelError> {
        println!("AhciController::init: enabling bus mastering");
        dev.enable_bus_mastering();
        let mut controller = AhciController::new(dev)?;
        controller.init(dev);
        println!(level:info, "AHCI controller initialized");
        println!("Ahci controller: {:#X?}", controller);
        for port in controller.ports.iter() {
            let future = crate::vfs::add_disk(port.clone());
            let ffi_future = ffi_future::future::into_ffi_future(future);
            block_task(ffi_future);
        }
        self.controller = MaybeUninit::new(controller);
        Ok(())
    }
    fn deinit(&mut self, _dev: &pci::LegacyPciDevice) {
        todo!("deinit for ahci not implemented yet")
    }
    fn remove_device(&mut self) {
        todo!("remove_device for ahci not implemented yet")
    }
    fn service_interrupt(&mut self, _dev: &pci::LegacyPciDevice) {
        let controller = unsafe { self.controller.assume_init_ref() };
        controller.service_interrupt();
    }
}

#[derive(Debug)]
pub struct AhciController {
    pub ghc: RWSpinlock<GenericHostControlPtr<'static>>,
    ports: Vec<Arc<VirtualPort>>,
    is_64_bit: bool,
}

impl AhciController {
    fn new(device: &pci::LegacyPciDevice) -> Result<Self, KernelError> {
        let abar = device
            .bars
            .iter()
            .find(|bar| bar.get_index() == 5)
            .ok_or(kerror_unwrapped!(IllegalValue))?;
        let pci::Bar::Memory(abar) = abar else {
            return kerror!(IllegalValue);
        };

        let ghc = unsafe { GenericHostControlPtr::from_ptr(abar.get_address().start.0 as *mut GenericHostControl) };
        let is_64_bit = ghc.cap().read().S64A();

        Ok(Self {
            ghc: RWSpinlock::new(ghc),
            ports: Vec::new(),
            is_64_bit,
        })
    }

    //https://forum.osdev.org/viewtopic.php?t=40969
    fn init(&mut self, _device: &pci::LegacyPciDevice) {
        let ghc_lock = w_lock_w_info!(self.ghc);
        println!("AhciController::init: staring ahci init");
        println!("AhciController::init: abar at {:p}", ghc_lock.as_ptr());
        let ghc_dbg = unsafe { ghc_lock.as_ptr().read_volatile() };
        println!("AhciController::init: ghc before init: {:#x?}", ghc_dbg);

        let mut ports = Vec::new();
        let ports_implemented = ghc_lock.pi().read();

        for i in 0..32 {
            if ports_implemented & (1 << i) != 0 {
                ports.push(VirtualPort {
                    index: i as u8,
                    address: (ghc_lock.as_ptr() as u64 + 0x100 + (i as u64) * 0x80) as *mut u32,
                    command_list: OwnedVirtAddr(VirtAddr(0)),
                    fis: VirtAddr(0),
                    is_64_bit: self.is_64_bit,
                    sectors: 0,
                    command_depth: 1,
                    device: 0,
                    commands_issued: AtomicU32::new(0),
                    address_lock: NoIntSpinlock::new(()),
                    task_wakers: array::from_fn(|_| NoIntSpinlock::new(None)),
                });
            }
        }

        println!("ports implemented: {:#x}", ghc_lock.pi().read());

        //enable AHCI
        ghc_lock.ghc().write(*ghc_lock.ghc().read().SetAE(true).SetIE(false));

        //bios handoff??
        if ghc_lock.cap2().read().BOH() {
            self.perform_bios_handoff();
        } else {
            println!("No bios handoff");
        }

        self.wait_for_idle_ports(&ports);

        //reset HBA
        ghc_lock.ghc().write(*ghc_lock.ghc().read().SetHR(true));
        while ghc_lock.ghc().read().HR() {
            std::thread::sleep(Duration::from_micros(10));
        }

        //enable AHCI again after reset
        ghc_lock.ghc().write(*ghc_lock.ghc().read().SetAE(true).SetIE(false));

        self.wait_for_idle_ports(&ports);

        let staggered_spin_up = ghc_lock.cap().read().SSS();

        let mut active_ports = Vec::new();
        //loop and init ports
        for port in &mut ports {
            if port.init(self.is_64_bit, staggered_spin_up) {
                active_ports.push(port.index);
            }
        }

        ghc_lock.is().write(0); //clear all interrupts
        ghc_lock.ghc().write(*ghc_lock.ghc().read().SetIE(true)); //enable global interrupts

        ports.retain(|port| active_ports.contains(&port.index));
        println!("Final ports: {:#x?}", ports);
        self.ports = ports.into_iter().map(Arc::new).collect();
    }

    fn perform_bios_handoff(&self) {
        let ghc_lock = w_lock_w_info!(self.ghc);
        let mut bohc = Bohc(0);
        bohc.SetOOS(true);
        println!("bohc: {:#x?}", bohc);
        ghc_lock.bohc().write(bohc);
        let start = std::time::Instant::now();
        loop {
            let bohc = ghc_lock.bohc().read();
            if bohc.BB() {
                loop {
                    let bohc = ghc_lock.bohc().read();
                    if !bohc.BB() || start.elapsed().as_secs() > 2 {
                        break;
                    }
                    std::thread::sleep(Duration::from_micros(10));
                }
                println!("Bios handoff complete");
                break;
            }
            if start.elapsed().as_millis() > 25 {
                println!("Bios handoff timeout");
                break;
            }
            std::thread::sleep(Duration::from_micros(10));
        }
    }

    fn wait_for_idle_ports(&self, ports: &Vec<VirtualPort>) {
        for port in ports {
            let mut port_command = PortCommand(port.get_property(0x18));
            if port_command.ST() {
                port_command.SetST(false);
                port.set_property(0x18, port_command.0);
                std::thread::sleep(Duration::from_micros(10));
            }
            while port_command.CR() {
                std::thread::sleep(Duration::from_micros(10));
                port_command = PortCommand(port.get_property(0x18));
            }
            if port_command.FR() {
                port_command.SetFRE(false);
                port.set_property(0x18, port_command.0);
                while port_command.FR() {
                    std::thread::sleep(Duration::from_micros(10));
                    port_command = PortCommand(port.get_property(0x18));
                }
            }
            let mut sctl = SATAControl(port.get_property(0x2C));
            if sctl.DET() != 0 {
                sctl.SetDet(0);
                port.set_property(0x2C, sctl.0);
            }
        }
    }

    fn service_interrupt(&self) {
        let ghc_lock = w_lock_w_info!(self.ghc);
        let is = ghc_lock.is().read();
        for port in &self.ports {
            if is & (1 << port.index) != 0 {
                //handle port interrupt
                port.service_interrupt();
            }
        }
        ghc_lock.is().write(is); //clear interrupts
        ghc_lock.is().read(); //read to ensure write arrived
        drop(ghc_lock);
    }
}

#[derive(Debug)]
struct VirtualPort {
    // commands_issued_addr_lock: Arc<(AtomicU32, NoIntSpinlock<()>)>,
    commands_issued: AtomicU32,
    task_wakers: [NoIntSpinlock<Option<Waker>>; 32],
    is_64_bit: bool,
    index: u8,
    //use lock
    address_lock: NoIntSpinlock<()>,
    address: *mut u32,
    sectors: u64,
    //thread safe (only written during init)
    fis: VirtAddr,
    //thread safe (as long as commands_issued works)
    command_list: OwnedVirtAddr,
    command_depth: u16,
    device: u8,
}

// Safe because we set all the data once, then only modify data in Arc<AtomicU32> and using the lock
unsafe impl Send for VirtualPort {}
unsafe impl Sync for VirtualPort {}

#[derive(Debug, Clone, Copy)]
struct CommandMetadata {
    issued: bool,
}

impl VirtualPort {
    pub fn get_command_index(&self) -> Option<u8> {
        let mut index = u8::MAX;
        self.commands_issued
            .fetch_update(
                core::sync::atomic::Ordering::AcqRel,
                core::sync::atomic::Ordering::Acquire,
                |current| {
                    index = current.trailing_ones() as u8;
                    if index >= self.command_depth as u8 {
                        return None;
                    }
                    Some(current | (1 << index))
                },
            )
            .ok()?;

        Some(index)
    }

    pub fn release_command_index(&self, index: u8) {
        self.commands_issued
            .fetch_and(!(1_u32 << index), core::sync::atomic::Ordering::AcqRel);
    }

    pub fn init_cmd_list_fis(&mut self, is_64_bit: bool) {
        const FIS_SWITCHING: bool = false;

        let cmd_list_base = if is_64_bit {
            physical_allocator::allocate()
        } else {
            panic!("i do not support 32 bit");
        };

        let fis_base = if !FIS_SWITCHING {
            cmd_list_base.0 + PhysAddr(0x400)
        } else {
            todo!("support fis switching maybe");
        };
        // } else if is_64_bit {
        //     physical_allocator::allocate()
        // } else {
        //     panic!("i do not support 32 bit");
        // };

        let lock = lock_w_info!(self.address_lock);
        self.set_property(0, cmd_list_base.0.0 as u32);
        self.set_property(4, (cmd_list_base.0.0 >> 32) as u32);
        self.set_property(8, fis_base.0 as u32);
        self.set_property(12, (fis_base.0 >> 32) as u32);
        drop(lock);

        let mut clb_virt = memory::kernel_map(cmd_list_base);

        unsafe { memset_at_addr(clb_virt.0, 0, 0x1000) };
        let fis_virt = if !FIS_SWITCHING {
            clb_virt.0 + 0x400_u64
        } else {
            todo!("support fis switching maybe");
            // let tmp_virt = memory::kernel_map(fis_base);
            // unsafe { memset_at_addr(tmp_virt, 0, 0x1000) };
            // tmp_virt
        };

        core::mem::swap(&mut self.command_list, &mut clb_virt);
        core::mem::forget(clb_virt); //now uninitialized data

        self.fis = fis_virt;
    }

    fn set_property(&self, offset: u64, value: u32) {
        unsafe { self.address.byte_add(offset as usize).write_volatile(value) };
    }

    fn get_property(&self, offset: u64) -> u32 {
        unsafe { self.address.byte_add(offset as usize).read_volatile() }
    }

    fn get_port(&self) -> Port {
        unsafe { (self.address as *const Port).read_volatile() }
    }

    fn display_port(&self) {
        println!("{:#x?}", self.get_port());
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    fn init(&mut self, is_64_bit: bool, staggered_spin_up: bool) -> bool {
        self.init_cmd_list_fis(is_64_bit);
        let mut port_cmd = PortCommand(self.get_property(0x18));
        port_cmd.SetFRE(true);
        self.set_property(0x18, port_cmd.0);
        //here a register FIS is sent immediately

        while !port_cmd.FR() {
            std::thread::sleep(Duration::from_micros(10));
            port_cmd = PortCommand(self.get_property(0x18));
        }

        port_cmd.SetST(true);
        self.set_property(0x18, port_cmd.0);

        if staggered_spin_up {
            println!("Staggered spin up");
            port_cmd.SetSUD(true);
            self.set_property(0x18, port_cmd.0);
        }

        //wait for port to be ready
        let mut sata_status = SATAStatus(self.get_property(0x28));
        let start = std::time::Instant::now();
        while sata_status.DET() != 3 {
            if start.elapsed().as_millis() > 10 {
                println!("Port {} not working", self.index);
                return false;
            }
            std::thread::sleep(Duration::from_micros(10));
            sata_status = SATAStatus(self.get_property(0x28));
        }
        //clear error register
        self.set_property(0x30, 0xFFFFFFFF);

        //wait for device to be ready
        let mut task_file_data = TaskFileData(self.get_property(0x20));
        while task_file_data.STS_BSY() || task_file_data.STS_DRQ() || task_file_data.STS_ERR() {
            std::thread::sleep(Duration::from_micros(10));
            task_file_data = TaskFileData(self.get_property(0x20));
        }

        if self.send_identify().is_err() {
            println!("Port {} identify failed", self.index);
            return false;
        }

        //clear interrupt status
        self.set_property(0x10, 0xFFFFFFFF);
        //enable port interrupts here (only register fis)
        self.set_property(0x14, 1);

        unsafe {
            let register_fis = &raw const *get_at_addr::<D2HRegisterFis, _>(self.fis + 0x40_u64);
            let _pio_setup_fis = &raw const *get_at_addr::<PioSetupFis, _>(self.fis + 0x20_u64);
            self.set_property(0x10, 3);
            self.device = register_fis.read_volatile().device;
            //use them?
        }

        println!("Port {} initialized", self.index);

        true
    }

    fn send_identify(&mut self) -> Result<(), KernelError> {
        let mut pmport = H2DRegFisPmport(0);
        pmport.set_command(true);
        let ident_fis = H2DRegisterFis {
            fis_type: FisType::RegisterH2D as u8,
            command: 0xEC, //identify
            pmport,
            device: 0xA0, // change depending on SATA/ATAPI
            control: 0x08,
            ..Default::default()
        };

        let fis_recv_area = physical_allocator::allocate();
        let fis_recv_borrowed = fis_recv_area.0;
        let prdt = PrdtDescriptor {
            base: fis_recv_area.0,
            count: 512,
        };

        let ident_fis = unsafe { core::mem::transmute::<H2DRegisterFis, [u8; 20]>(ident_fis) };
        let identify_cmd_index = self.get_command_index().expect("no command slots free during identify?????");
        self.build_command(false, &ident_fis, &[prdt], identify_cmd_index);

        let now = std::time::Instant::now();

        let mut ci = self.get_property(0x38);
        while ci & (1 << identify_cmd_index) != 0 {
            if now.elapsed().as_millis() > 50 {
                println!("Identify command timeout");
                self.clean_command(identify_cmd_index);
                self.release_command_index(identify_cmd_index);
                return kerror!(Timeout);
            }
            std::thread::sleep(Duration::from_micros(10));
            ci = self.get_property(0x38);
        }

        self.clean_command(identify_cmd_index);
        self.release_command_index(identify_cmd_index);

        unsafe {
            let data = &raw const *get_at_addr::<IdentifyStructure, _>(fis_recv_borrowed);
            let data = data.read_volatile();

            self.sectors = data.total_usr_sectors();
            self.command_depth = data.queue_depth;
            assert!(data.sector_bytes == 512);
        }
        drop(fis_recv_area); //explicit late drop

        Ok(())
    }

    ///PRDT cannot be more than a bit over 900MB. Just use multiple commands
    fn build_command(&self, write: bool, cfis: &[u8], prdt: &[PrdtDescriptor], index: u8) {
        assert!(prdt.len() <= 248); //i don't want to deal with contiguous allocation

        let cmd_table_frame = if self.is_64_bit {
            physical_allocator::allocate()
        } else {
            panic!("i do not support 32 bit");
        };

        let mut cmd_header = CmdHeader(0);
        cmd_header.SetWrite(write);
        cmd_header.SetCFL(cfis.len() as u128 / 4);
        cmd_header.SetClearBusy(true);
        cmd_header.SetPRDTL(prdt.len() as u128);
        debug_assert!(cmd_table_frame.0.0 & 0b1111111 == 0); //128 byte alignment
        cmd_header.SetCTBA(cmd_table_frame.0.0 as u128);

        unsafe {
            let cmd_header_ptr = (self.command_list.0.0 as *mut CmdHeader).add(index as usize * 4);
            cmd_header_ptr.write_volatile(cmd_header);

            let cmd_table_virt = VirtAddr::from(cmd_table_frame.0);
            core::mem::forget(cmd_table_frame); //don't free this memory, it's used by the controller
            let cmd_table_raw = cmd_table_virt.0 as *mut u8;
            for (i, byte) in cfis.iter().enumerate() {
                cmd_table_raw.add(i).write_volatile(*byte);
            }

            for (i, prdt) in prdt.iter().enumerate() {
                let prdt_entry_ptr = cmd_table_raw.add(0x80 + i * 16) as *mut PrdtEntry;
                let mut prdt_entry = PrdtEntry(0);
                prdt_entry.SetInt(true);
                prdt_entry.SetDBA(prdt.base.0.into());
                prdt_entry.SetDBC(prdt.count as u128 - 1);
                prdt_entry_ptr.write_volatile(PrdtEntry(prdt_entry.0));
            }
        }

        let cmd_issue = 1 << index;

        //no need for lock, is write-1 register
        self.set_property(0x38, cmd_issue);
    }

    ///frees command header memory. Does not free regions pointed to by PRDT
    fn clean_command(&self, index: u8) {
        unsafe {
            let cmd_header = (self.command_list.0.0 as *mut u32).add(index as usize * 4);
            let table_lower = cmd_header.add(2).read_volatile();
            let table_upper = cmd_header.add(3).read_volatile();
            let table = (table_upper as u64) << 32 | table_lower as u64;
            physical_allocator::deallocate(OwnedPhysAddr(PhysAddr(table)));
        }
        //potentially anything else
    }

    pub fn is_command_ready(&self, command_slot: u8) -> bool {
        let lock = lock_w_info!(self.address_lock);
        let ci = self.get_property(0x38);
        drop(lock);
        ci & (1 << command_slot) == 0
    }

    fn service_interrupt(&self) {
        let lock = lock_w_info!(self.address_lock);
        let is = self.get_property(0x10);
        self.set_property(0x10, is); //clear interrupts
        self.get_property(0x10); //read to ensure write arrived
        let in_service = self.get_property(0x38);
        let issued = self.commands_issued.load(core::sync::atomic::Ordering::Acquire);
        let completed = issued & !in_service;
        self.commands_issued
            .fetch_and(!completed, core::sync::atomic::Ordering::AcqRel);
        drop(lock);

        let serr = self.get_property(0x30);
        if serr != 0 {
            println!("Port {} SERR: {:#x}", self.index, serr);
            self.set_property(0x30, serr); //clear errors
            self.get_property(0x30); //read to ensure write arrived
        }

        // TODO: check register fis for errors

        for i in 0..self.command_depth {
            if completed & (1 << i) != 0 {
                let mut waker_lock = lock_w_info!(self.task_wakers[i as usize]);
                if let Some(waker) = waker_lock.deref_mut().take() {
                    waker.wake();
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl BlockDevice for VirtualPort {
    async fn read(&self, start_sec_index: usize, sec_count: usize, buffer: &[PhysAddr]) {
        println!(level:debug, "Reading from ahci disk, sector {}, n sectors {}", start_sec_index, sec_count);

        OPERATIONS.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        assert!(sec_count <= self.sectors as usize);
        let prdt_entries = sec_count.div_ceil(8); //8 sectors in one physical frame

        let prdt = buffer
            .iter()
            .enumerate()
            .map(|(i, addr)| {
                PrdtDescriptor {
                    base: *addr,
                    count: if i == prdt_entries - 1 {
                        (((sec_count - 1) as u32 % 8) + 1) * 512
                    } else {
                        //4K byte regions
                        8 * 512
                    },
                }
            })
            .collect::<Vec<_>>();

        let mut pmport = H2DRegFisPmport(0);
        pmport.set_command(true);

        let cfis = H2DRegisterFis {
            pmport,
            command: READ_DMA,
            device: self.device | (1 << 6),
            countl: sec_count as u8,
            counth: (sec_count >> 8) as u8,
            lba0: (start_sec_index >> 0) as u8,
            lba1: (start_sec_index >> 8) as u8,
            lba2: (start_sec_index >> 16) as u8,
            lba3: (start_sec_index >> 24) as u8,
            lba4: (start_sec_index >> 32) as u8,
            lba5: (start_sec_index >> 40) as u8,
            ..H2DRegisterFis::default()
        };

        let read_cmd_index = loop {
            match self.get_command_index() {
                Some(cmd_index) => break cmd_index,
                None => {
                    task_runner::yield_now().await;
                }
            }
        };
        self.build_command(false, (&cfis).into(), &prdt, read_cmd_index);
        CommandWaiter {
            port: self,
            command_index: read_cmd_index,
        }
        .await;

        self.clean_command(read_cmd_index);
        self.release_command_index(read_cmd_index);
    }

    ///Returns the virtual address of the read data and the command index used
    async fn write(&self, start_sec_index: usize, sec_count: usize, buffer: &[PhysAddr]) {
        println!("writing to disk at sector {} with sec count {}", start_sec_index, sec_count);

        OPERATIONS.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        assert!(sec_count <= self.sectors as usize);
        let prdt_entries = sec_count.div_ceil(8); //8 sectors in one physical frame

        let prdt = buffer
            .iter()
            .enumerate()
            .map(|(i, addr)| {
                PrdtDescriptor {
                    base: *addr,
                    count: if i == prdt_entries - 1 {
                        (((sec_count - 1) as u32 % 8) + 1) * 512
                    } else {
                        //4K byte regions
                        8 * 512
                    },
                }
            })
            .collect::<Vec<_>>();

        let mut pmport = H2DRegFisPmport(0);
        pmport.set_command(true);

        let cfis = H2DRegisterFis {
            pmport,
            command: WRITE_DMA,
            device: self.device | (1 << 6),
            countl: sec_count as u8,
            counth: (sec_count >> 8) as u8,
            lba0: (start_sec_index >> 0) as u8,
            lba1: (start_sec_index >> 8) as u8,
            lba2: (start_sec_index >> 16) as u8,
            lba3: (start_sec_index >> 24) as u8,
            lba4: (start_sec_index >> 32) as u8,
            lba5: (start_sec_index >> 40) as u8,
            ..H2DRegisterFis::default()
        };

        println!("created fis");

        let write_cmd_index = loop {
            match self.get_command_index() {
                Some(cmd_index) => break cmd_index,
                None => {
                    task_runner::yield_now().await;
                }
            }
        };

        println!("got cmd index {}", write_cmd_index);

        self.build_command(false, (&cfis).into(), &prdt, write_cmd_index);

        println!("dispatched command");

        CommandWaiter {
            port: self,
            command_index: write_cmd_index,
        }
        .await;

        self.clean_command(write_cmd_index);
        self.release_command_index(write_cmd_index);

        println!("released cmd index");
    }
}

pub fn clear_operations_count() {
    OPERATIONS.store(0, core::sync::atomic::Ordering::Release);
}

pub fn get_operations_count() -> u32 {
    OPERATIONS.load(core::sync::atomic::Ordering::Acquire)
}

struct CommandWaiter<'a> {
    port: &'a VirtualPort,
    command_index: u8,
}

impl Future for CommandWaiter<'_> {
    type Output = ();

    fn poll(self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let waker = cx.waker().clone();
        let cmd_index = self.command_index as usize;
        //lock early to prevent interrupt after check but before setting waker
        let mut task_waker_lock = lock_w_info!(self.port.task_wakers[cmd_index]);
        if self.port.is_command_ready(self.command_index) {
            core::task::Poll::Ready(())
        } else {
            *task_waker_lock.deref_mut() = Some(waker);
            core::task::Poll::Pending
        }
    }
}

bitfield! {
    struct CmdHeader(u128);
    impl Debug;
    CFL, SetCFL: 4, 0;
    Atapi, SetAtapi: 5;
    Write, SetWrite: 6;
    Prefetchable, SetPrefetchable: 7;
    Reset, SetReset: 8;
    Bist, SetBist: 9;
    ClearBusy, SetClearBusy: 10;
    PMP, SetPMP: 15, 12;
    PRDTL, SetPRDTL: 31, 16;
    PRDBC, SetPRDBC: 63, 32;
    CTBA, SetCTBA: 127, 64;

}

struct PrdtDescriptor {
    base: PhysAddr,
    count: u32,
}

bitfield! {
    struct PrdtEntry(u128);
    impl Debug;
    DBA, SetDBA: 63, 0;
    DBC, SetDBC: 117, 96;
    Int, SetInt: 127;
}

#[derive(Debug)]
#[repr(C)]
struct Port {
    PxCLB: u64,
    PxFB: u64,
    PxIS: u32,
    PxIE: u32,
    ///WARNING! contains RW1 field
    PxCMD: PortCommand,
    reserved: u32,
    PxTFD: TaskFileData,
    PxSIG: u32,
    PxSSTS: SATAStatus,
    PxSCTL: SATAControl,
    PxSERR: u32,
    PxSACT: u32,
    PxCI: u32,
    PxSNTF: u32,
    PxFBS: u32,
    PxDEVSLP: u32,
    reserved2: [u32; 10],
    PxVS: u32,
}

#[derive(Debug, RegMap)]
#[repr(C)]
pub struct GenericHostControl {
    cap: Capabilities,
    ghc: GlobalHBAControl,
    is: u32,
    pi: u32,
    vs: u32,
    ccc_ctl: u32,
    ccc_ports: u32,
    em_loc: u32,
    em_ctl: u32,
    cap2: Capabilities2,
    ///WARNING! containes RWC field
    bohc: Bohc,
}

unsafe impl Send for GenericHostControlPtr<'_> {}

bitfield! {
    #[derive(RegMap)]
    struct GlobalHBAControl(u32);
    impl Debug;
    AE, SetAE: 31;
    MRSM, _: 2;
    IE, SetIE: 1;
    /// SetOOC write 1 to set
    HR, SetHR: 0;
}

bitfield! {
    #[derive(RegMap)]
    struct Capabilities(u32);
    impl Debug;
    S64A, _: 31;
    SSS, _: 27;
}

bitfield! {
    #[derive(RegMap)]
    struct Capabilities2(u32);
    impl Debug;
    DESO, _: 5;
    SADM, _: 4;
    SDS, _: 3;
    APST, _: 2;
    NVMP, _: 1;
    BOH, _: 0;
}

bitfield! {
    #[derive(RegMap)]
    struct Bohc(u32);
    impl Debug;
    BB, SetBB: 4;
    /// SetOOC write 1 to clear
    OOC, SetOOC: 3;
    SOOE, SetSOOE: 2;
    OOS, SetOOS: 1;
    BOS, SetBOS: 0;
}

bitfield! {
    struct PortCommand(u32);
    impl Debug;
    CR, _: 15;
    FR, _: 14;
    FRE, SetFRE: 4;
    /// RW1
    CLO, SetClo: 3;
    ///Before setting, set CLO and wait for it to clear
    SUD, SetSUD: 1;
    ST, SetST: 0;
}

bitfield! {
    struct TaskFileData(u32);
    impl Debug;
    ERR, _: 15, 8;
    STS_BSY, _: 7;
    STS_DRQ, _: 3;
    STS_ERR, _: 0;
}

bitfield! {
    struct SATAStatus(u32);
    impl Debug;
    IPM, _: 11, 8;
    SPD, _: 7, 4;
    DET, _: 3, 0;
}

bitfield! {
    struct SATAControl(u32);
    impl Debug;
    DET, SetDet: 3, 0;
}

#[derive(Debug)]
#[repr(C, packed)]
struct CommandHeader {
    dw0: u32,
    dw1: u32,
    dw2: u32,
    dw3: u32,
}

impl CommandHeader {
    pub fn new() -> Self {
        Self {
            dw0: 0,
            dw1: 0,
            dw2: 0,
            dw3: 0,
        }
    }
}
