#![allow(clippy::enum_variant_names)]
#![allow(clippy::needless_range_loop)]

use std::{
    mem_utils::{PhysAddr, VirtAddr},
    println,
    vec::Vec,
};

use crate::memory::{PAGE_TREE_ALLOCATOR, paging::LiminePat, physical_allocator};

use super::port_access;

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone)]
pub struct RegularPciDevice {
    pub device: PciDevice,
    pub bars: Vec<Bar>,
    //more fields?
}

impl RegularPciDevice {
    pub fn new(device: PciDevice) -> Self {
        let mut bars = Vec::new();
        let mut i = 0;

        //disconnect device from any BARs
        let command = device.get_command();
        device.set_command(command & !0x3);

        while i < 6 {
            let bar = device.get_bar(i);
            if let Some(bar) = bar {
                bars.push(bar.0);
                i += bar.1;
            } else {
                i += 1;
            }
        }
        device.set_command(command);
        Self { device, bars }
    }

    pub fn enable_bus_mastering(&self) {
        let command = self.device.get_command();
        self.device.set_command(command | 0b100);
    }
}

#[derive(Debug, Clone)]
pub enum Bar {
    Memory(u8, VirtAddr, u64),
    IO(u8, u16, u32),
}

impl Bar {
    pub fn write_to_bar<T>(&self, data: &T, offset: u64) {
        let data = unsafe { core::slice::from_raw_parts(data as *const T as *const u8, core::mem::size_of::<T>()) };
        match self {
            Bar::Memory(_, address, limit) => {
                let address = (address.0 + offset) as *mut u8;
                assert!(offset + data.len() as u64 <= *limit, "Data exceeds BAR size");
                unsafe {
                    for i in 0..data.len() {
                        address.add(i).write_volatile(data[i]);
                    }
                }
            }
            Bar::IO(_, address, limit) => {
                let address = *address + offset as u16;
                assert!(offset + data.len() as u64 <= *limit as u64, "Data exceeds BAR size");
                for i in 0..data.len() {
                    crate::utils::byte_to_port(address + i as u16, data[i]);
                }
            }
        }
    }

    pub fn read_from_bar<T: Sized>(&self, offset: u64) -> T {
        let mut data = Vec::with_capacity(core::mem::size_of::<T>());
        match self {
            Bar::Memory(_, address, limit) => {
                let address = (address.0 + offset) as *const u8;
                assert!(offset + data.len() as u64 <= *limit, "Data exceeds BAR size");
                unsafe {
                    for i in 0..core::mem::size_of::<T>() {
                        let byte = address.add(i).read_volatile();
                        data.push(byte);
                    }
                }
            }
            Bar::IO(_, address, limit) => {
                let address = *address + offset as u16;
                assert!(offset + data.len() as u64 <= *limit as u64, "Data exceeds BAR size");
                for i in 0..data.len() {
                    data.push(crate::utils::byte_from_port(address + i as u16));
                }
            }
        }
        unsafe { core::ptr::read(data.as_ptr() as *const T) }
    }

    pub fn get_index(&self) -> u8 {
        match self {
            Bar::Memory(index, _, _) => *index,
            Bar::IO(index, _, _) => *index,
        }
    }
}

impl PciDevice {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        Self {
            bus,
            device,
            function,
            capabilities: Vec::new(),
        }
    }

    pub fn get_dword(&self, offset: u8) -> u32 {
        port_access::get_dword(self.bus, self.device, self.function, offset)
    }

    pub fn set_dword(&self, offset: u8, value: u32) {
        port_access::set_dword(self.bus, self.device, self.function, offset, value)
    }

    pub fn get_vendor_id(&self) -> u16 {
        self.get_dword(0) as u16
    }

    pub fn get_device_id(&self) -> u16 {
        (self.get_dword(0) >> 16) as u16
    }

    pub fn get_command(&self) -> u16 {
        self.get_dword(4) as u16
    }

    pub fn set_command(&self, value: u16) {
        self.set_dword(4, value as u32);
    }

    pub fn get_status(&self) -> u16 {
        (self.get_dword(4) >> 16) as u16
    }

    pub fn get_revision_id(&self) -> u8 {
        self.get_dword(8) as u8
    }

    pub fn get_progif(&self) -> u8 {
        (self.get_dword(8) >> 8) as u8
    }

    pub fn get_class(&self) -> PciClass {
        let class_subclass = self.get_dword(8) >> 16;
        let class = (class_subclass >> 8) as u8;
        let subclass = class_subclass as u8;
        PciClass::from(class, subclass)
    }

    pub fn get_header_type(&self) -> u8 {
        (self.get_dword(0xC) >> 16) as u8
    }

    pub fn get_bist(&self) -> u8 {
        (self.get_dword(0xC) >> 24) as u8
    }

    pub fn get_latency_timer(&self) -> u8 {
        (self.get_dword(0xC) >> 8) as u8
    }

    pub fn get_cache_line_size(&self) -> u8 {
        self.get_dword(0xC) as u8
    }

    fn get_bar(&self, index: u8) -> Option<(Bar, u8)> {
        #[cfg(debug_assertions)]
        {
            let header_type = self.get_header_type() & 0x7F;
            if header_type == 0 {
                assert!(index < 6, "Invalid BAR index for header type 0: {}", index);
            } else if header_type == 1 {
                assert!(index < 2, "Invalid BAR index for header type 1: {}", index);
            } else {
                panic!("Header type {} does not conatin BARs", header_type);
            }
        }

        let first_bar = self.get_dword(0x10 + index * 4);
        if first_bar == 0 {
            return None;
        }
        if first_bar & 0x1 == 0 {
            //memory space bar
            let physical_bar_addr: PhysAddr;
            let size: u64;
            let bars: u8;
            let prefetchable = (first_bar & 0b1000) != 0;
            if first_bar & 0b100 != 0 {
                let second_bar = self.get_dword(0x10 + index * 4 + 4);
                let address = (first_bar & 0xFFFF_FFF0) as u64 | ((second_bar as u64) << 32);
                physical_bar_addr = PhysAddr(address);
                bars = 2;
                size = (self.get_bar_size(index, 0xF) as u64) | ((self.get_bar_size(index + 1, 0) as u64) << 32);
            } else {
                physical_bar_addr = PhysAddr(first_bar as u64 & 0xFFFF_FFF0);
                bars = 1;
                size = self.get_bar_size(index, 0xF) as u64;
            }
            let num = size / 4096;
            let address = unsafe {
                for i in 0..num {
                    physical_allocator::mark_addr(physical_bar_addr + PhysAddr(i * 0x1000), true);
                }
                let address = PAGE_TREE_ALLOCATOR.allocate_contigious(num, Some(physical_bar_addr), false);
                println!(
                    "Mapped memory BAR from virt {:#X} to phys {:#X} with size {:#X}",
                    address.0, physical_bar_addr.0, size
                );
                //mark caching as uncacheable, unless prefetchable, then write-through
                for i in 0..num {
                    let page_entry = PAGE_TREE_ALLOCATOR.get_page_table_entry_mut(address + (i * 4096)).expect("could not retrieve just mapped addresses");
                    if prefetchable {
                        page_entry.set_pat(LiminePat::WT);
                    } else {
                        page_entry.set_pat(LiminePat::UC);
                    }
                }

                address
            };
            Some((Bar::Memory(index, address, size), bars))
        } else {
            //io space bar
            let address = first_bar as u16 & 0xFFFC;
            let size = self.get_bar_size(index, 0x3);
            Some((Bar::IO(index, address, size), 1))
        }
    }

    fn get_bar_size(&self, index: u8, mask: u32) -> u32 {
        let bar = self.get_dword(0x10 + index * 4);
        self.set_dword(0x10 + index * 4, 0xFFFF_FFFF);
        let size = self.get_dword(0x10 + index * 4) & !mask;
        self.set_dword(0x10 + index * 4, bar);
        (!size) + 1
    }

    pub fn get_capabilities_pointer(&self) -> u8 {
        (self.get_dword(0x34) & 0b11111100) as u8
    }

    pub fn get_capabilities_list(&mut self) -> &Vec<Capability> {
        let status = self.get_status();
        if (status & 0x10) == 0 {
            return &self.capabilities;
        }
        let mut capabilities = Vec::new();
        let mut pointer = self.get_capabilities_pointer();
        while pointer != 0 {
            let capability_first_dword = self.get_dword(pointer);
            let capability_id = capability_first_dword as u8;
            capabilities.push(Capability {
                id: capability_id,
                pointer,
            });
            pointer = (capability_first_dword >> 8) as u8;
        }
        self.capabilities = capabilities;
        &self.capabilities
    }

    //MSI functions
    pub fn get_msi_64_bit(&self, msi_cap: &Capability) -> bool {
        let dword = self.get_dword(msi_cap.pointer) >> 16;
        (dword & 0x80) != 0
    }

    pub fn init_msi_interrupt(&self) {
        //disable INTx# interrupts (pins?)
        let command = self.get_command();
        self.set_command(command & !0x400);

        let capabilities = &self.capabilities;
        let msi_cap = capabilities
            .iter()
            .find(|cap| cap.id == 5)
            .cloned()
            .expect("Device does not support MSI");
        let is_64_capable = self.get_msi_64_bit(&msi_cap);
        let first_dword = self.get_dword(msi_cap.pointer);
        let mut message_control = (first_dword >> 16) as u16;

        //get number of requested interrupts, and allow max...?
        let requested_interrupts_power = u16::min((message_control & 0b1110) >> 1, 5);
        let requested_interrupts = 1 << requested_interrupts_power;
        println!("Requested interrupts: {}", requested_interrupts);
        message_control &= !0b1110000;
        //give number of vectors
        message_control |= requested_interrupts_power << 4;

        //get mext available irq with bottom bits set to 0
        let mut current_free_irq = unsafe { crate::interrupts::idt::CUSTOM_INTERRUPT_VECTOR };
        current_free_irq += requested_interrupts - 1;
        current_free_irq &= !(requested_interrupts - 1);
        self.set_msi_address(&msi_cap, is_64_capable);

        let data_dword_offset = if is_64_capable { 0xC } else { 0x8 };
        let data_dword = self.get_dword(msi_cap.pointer + data_dword_offset);
        self.set_dword(
            msi_cap.pointer + data_dword_offset,
            data_dword & 0xFFFF_0000 | current_free_irq as u32,
        );

        unsafe {
            crate::interrupts::idt::CUSTOM_INTERRUPT_VECTOR = current_free_irq + 1;
        }

        //enable MSI
        message_control |= 0x1;

        self.set_dword(msi_cap.pointer, (message_control as u32) << 16 | (first_dword & 0xFFFF));
    }

    fn set_msi_address(&self, msi_cap: &Capability, is_64_bit: bool) {
        let platform_info = crate::acpi::get_platform_info();
        let destination_mode = 0;
        let destination_id = platform_info.boot_processor.apic_id as u32;

        let irq_address = 0xFFE << 20 | destination_mode << 2 | destination_id << 12;

        let low_address = irq_address;
        let high_address = 0;

        self.set_dword(msi_cap.pointer + 4, low_address);
        if is_64_bit {
            self.set_dword(msi_cap.pointer + 8, high_address);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub id: u8,
    pub pointer: u8,
}

#[derive(Debug)]
pub enum PciClass {
    Unclassified(Unclassified),
    MassStorageController(MassStorageController),
    NetworkController(NetworkController),
    DisplayController(DisplayController),
    MultimediaController(MultimediaController),
    MemoryController(MemoryController),
    BridgeDevice(BridgeDevice),
    SimpleCommunicationController(SimpleCommunicationController),
    BaseSystemPeripheral(BaseSystemPeripheral),
    InputDeviceController(InputDeviceController),
    DockingStation,
    Processor(Processor),
    SerialBusController(SerialBusController),
    WirelessController(WirelessController),
    IntelligentController,
    SatelliteCommunicationController(SatelliteCommunicationController),
    EncryptionController(EncryptionController),
    SignalProcessingController(SignalProcessingController),
    ProcessingAccelerator,
    NonEssentialInstrumentation,
    Coprocessor,
}

impl PciClass {
    pub fn from(class: u8, subclass: u8) -> Self {
        match class {
            0x00 => match subclass {
                0x00 => Self::Unclassified(Unclassified::NonVgaCompatibleDevice),
                0x01 => Self::Unclassified(Unclassified::VgaCompatibleDevice),
                _ => panic!("Invalid subclass for class 0x00: {:x}", subclass),
            },
            0x01 => match subclass {
                0x00 => Self::MassStorageController(MassStorageController::SCSIController),
                0x01 => Self::MassStorageController(MassStorageController::IDEController),
                0x02 => Self::MassStorageController(MassStorageController::FloppyDiskController),
                0x03 => Self::MassStorageController(MassStorageController::IPIController),
                0x04 => Self::MassStorageController(MassStorageController::RAIDController),
                0x05 => Self::MassStorageController(MassStorageController::ATAController),
                0x06 => Self::MassStorageController(MassStorageController::SerialATAController),
                0x07 => Self::MassStorageController(MassStorageController::SerialAttachedSCSIController),
                0x08 => Self::MassStorageController(MassStorageController::NonVolatileMemoryController),
                0x80 => Self::MassStorageController(MassStorageController::Other),
                _ => panic!("Invalid subclass for class 0x01: {:x}", subclass),
            },
            0x02 => match subclass {
                0x00 => Self::NetworkController(NetworkController::EthernetController),
                0x01 => Self::NetworkController(NetworkController::TokenRingController),
                0x02 => Self::NetworkController(NetworkController::FDDIController),
                0x03 => Self::NetworkController(NetworkController::ATMController),
                0x04 => Self::NetworkController(NetworkController::ISDNController),
                0x05 => Self::NetworkController(NetworkController::WorldFipController),
                0x06 => Self::NetworkController(NetworkController::PICMGController),
                0x07 => Self::NetworkController(NetworkController::InfinibandController),
                0x08 => Self::NetworkController(NetworkController::FabricController),
                0x80 => Self::NetworkController(NetworkController::Other),
                _ => panic!("Invalid subclass for class 0x02: {:x}", subclass),
            },
            0x03 => match subclass {
                0x00 => Self::DisplayController(DisplayController::VGACompatibleController),
                0x01 => Self::DisplayController(DisplayController::XGAController),
                0x02 => Self::DisplayController(DisplayController::ThreeDController),
                0x80 => Self::DisplayController(DisplayController::Other),
                _ => panic!("Invalid subclass for class 0x03: {:x}", subclass),
            },
            0x04 => match subclass {
                0x00 => Self::MultimediaController(MultimediaController::MultimediaVideoController),
                0x01 => Self::MultimediaController(MultimediaController::MultimediaAudioController),
                0x02 => Self::MultimediaController(MultimediaController::ComputerTelephonyDevice),
                0x03 => Self::MultimediaController(MultimediaController::AudioDevice),
                0x80 => Self::MultimediaController(MultimediaController::Other),
                _ => panic!("Invalid subclass for class 0x04: {:x}", subclass),
            },
            0x05 => match subclass {
                0x00 => Self::MemoryController(MemoryController::RAMController),
                0x01 => Self::MemoryController(MemoryController::FlashController),
                0x80 => Self::MemoryController(MemoryController::Other),
                _ => panic!("Invalid subclass for class 0x05: {:x}", subclass),
            },
            0x06 => match subclass {
                0x00 => Self::BridgeDevice(BridgeDevice::HostBridge),
                0x01 => Self::BridgeDevice(BridgeDevice::ISAbridge),
                0x02 => Self::BridgeDevice(BridgeDevice::EISAbridge),
                0x03 => Self::BridgeDevice(BridgeDevice::MCAbridge),
                0x04 => Self::BridgeDevice(BridgeDevice::PCItoPCIbridge),
                0x05 => Self::BridgeDevice(BridgeDevice::PCMCIAbridge),
                0x06 => Self::BridgeDevice(BridgeDevice::NuBusbridge),
                0x07 => Self::BridgeDevice(BridgeDevice::CardBusbridge),
                0x08 => Self::BridgeDevice(BridgeDevice::RACEwaybridge),
                0x09 => Self::BridgeDevice(BridgeDevice::PCItoPCIbridgeSemiTransparent),
                0x0A => Self::BridgeDevice(BridgeDevice::InfiniBandtoPCIHostBridge),
                0x80 => Self::BridgeDevice(BridgeDevice::Other),
                _ => panic!("Invalid subclass for class 0x06: {:x}", subclass),
            },
            0x07 => match subclass {
                0x00 => Self::SimpleCommunicationController(SimpleCommunicationController::SerialController),
                0x01 => Self::SimpleCommunicationController(SimpleCommunicationController::ParallelController),
                0x02 => Self::SimpleCommunicationController(SimpleCommunicationController::MultiportSerialController),
                0x03 => Self::SimpleCommunicationController(SimpleCommunicationController::Modem),
                0x04 => Self::SimpleCommunicationController(SimpleCommunicationController::GPIBController),
                0x05 => Self::SimpleCommunicationController(SimpleCommunicationController::SmardCardController),
                0x80 => Self::SimpleCommunicationController(SimpleCommunicationController::Other),
                _ => panic!("Invalid subclass for class 0x07: {:x}", subclass),
            },
            0x08 => match subclass {
                0x00 => Self::BaseSystemPeripheral(BaseSystemPeripheral::Pic),
                0x01 => Self::BaseSystemPeripheral(BaseSystemPeripheral::DMAController),
                0x02 => Self::BaseSystemPeripheral(BaseSystemPeripheral::Timer),
                0x03 => Self::BaseSystemPeripheral(BaseSystemPeripheral::Rtc),
                0x04 => Self::BaseSystemPeripheral(BaseSystemPeripheral::PCIHotPlugController),
                0x05 => Self::BaseSystemPeripheral(BaseSystemPeripheral::SDHostController),
                0x06 => Self::BaseSystemPeripheral(BaseSystemPeripheral::Iommu),
                0x80 => Self::BaseSystemPeripheral(BaseSystemPeripheral::Other),
                _ => panic!("Invalid subclass for class 0x08: {:x}", subclass),
            },
            0x09 => match subclass {
                0x00 => Self::InputDeviceController(InputDeviceController::KeyboardController),
                0x01 => Self::InputDeviceController(InputDeviceController::DigitizerPen),
                0x02 => Self::InputDeviceController(InputDeviceController::MouseController),
                0x03 => Self::InputDeviceController(InputDeviceController::ScannerController),
                0x04 => Self::InputDeviceController(InputDeviceController::GameportController),
                0x80 => Self::InputDeviceController(InputDeviceController::Other),
                _ => panic!("Invalid subclass for class 0x09: {:x}", subclass),
            },
            0x0A => Self::DockingStation,
            0x0B => match subclass {
                0x00 => Self::Processor(Processor::I386),
                0x01 => Self::Processor(Processor::I486),
                0x02 => Self::Processor(Processor::Pentium),
                0x10 => Self::Processor(Processor::Alpha),
                0x20 => Self::Processor(Processor::PowerPC),
                0x30 => Self::Processor(Processor::Mips),
                0x40 => Self::Processor(Processor::CoProcessor),
                0x80 => Self::Processor(Processor::Other),
                _ => panic!("Invalid subclass for class 0x0B: {:x}", subclass),
            },
            0x0C => match subclass {
                0x00 => Self::SerialBusController(SerialBusController::FireWireController),
                0x01 => Self::SerialBusController(SerialBusController::ACCESSBusController),
                0x02 => Self::SerialBusController(SerialBusController::Ssa),
                0x03 => Self::SerialBusController(SerialBusController::USBController),
                0x04 => Self::SerialBusController(SerialBusController::FibreChannelController),
                0x05 => Self::SerialBusController(SerialBusController::SMBus),
                0x06 => Self::SerialBusController(SerialBusController::InfiniBandController),
                0x07 => Self::SerialBusController(SerialBusController::IPMIController),
                0x80 => Self::SerialBusController(SerialBusController::Other),
                _ => panic!("Invalid subclass for class 0x0C: {:x}", subclass),
            },
            0x0D => match subclass {
                0x00 => Self::WirelessController(WirelessController::IRController),
                0x01 => Self::WirelessController(WirelessController::ConsumerIRController),
                0x10 => Self::WirelessController(WirelessController::RFController),
                0x11 => Self::WirelessController(WirelessController::BluetoothController),
                0x12 => Self::WirelessController(WirelessController::BroadbandController),
                0x20 => Self::WirelessController(WirelessController::EthernetController),
                0x80 => Self::WirelessController(WirelessController::Other),
                _ => panic!("Invalid subclass for class 0x0D: {:x}", subclass),
            },
            0x0E => Self::IntelligentController,
            0x0F => match subclass {
                0x00 => Self::SatelliteCommunicationController(SatelliteCommunicationController::TVController),
                0x01 => Self::SatelliteCommunicationController(SatelliteCommunicationController::AudioController),
                0x02 => Self::SatelliteCommunicationController(SatelliteCommunicationController::VoiceController),
                0x03 => Self::SatelliteCommunicationController(SatelliteCommunicationController::DataController),
                _ => panic!("Invalid subclass for class 0x0F: {:x}", subclass),
            },
            0x10 => match subclass {
                0x00 => Self::EncryptionController(EncryptionController::NetworkAndComputingEncryptionDevice),
                0x10 => Self::EncryptionController(EncryptionController::EntertainmentEncryptionDevice),
                0x80 => Self::EncryptionController(EncryptionController::Other),
                _ => panic!("Invalid subclass for class 0x10: {:x}", subclass),
            },
            0x11 => match subclass {
                0x00 => Self::SignalProcessingController(SignalProcessingController::DPIOmodule),
                0x01 => Self::SignalProcessingController(SignalProcessingController::PerformanceCounters),
                0x80 => Self::SignalProcessingController(SignalProcessingController::Other),
                _ => panic!("Invalid subclass for class 0x11: {:x}", subclass),
            },
            0x12 => Self::ProcessingAccelerator,
            0x13 => Self::NonEssentialInstrumentation,
            0x40 => Self::Coprocessor,
            _ => panic!("Invalid class: {}", class),
        }
    }
}

#[derive(Debug)]
pub enum Unclassified {
    NonVgaCompatibleDevice,
    VgaCompatibleDevice,
}

#[derive(Debug)]
pub enum MassStorageController {
    SCSIController,
    IDEController,
    FloppyDiskController,
    IPIController,
    RAIDController,
    ATAController,
    SerialATAController,
    SerialAttachedSCSIController,
    NonVolatileMemoryController,
    Other,
}

#[derive(Debug)]
pub enum NetworkController {
    EthernetController,
    TokenRingController,
    FDDIController,
    ATMController,
    ISDNController,
    WorldFipController,
    PICMGController,
    InfinibandController,
    FabricController,
    Other,
}

#[derive(Debug)]
pub enum DisplayController {
    VGACompatibleController,
    XGAController,
    ThreeDController,
    Other,
}

#[derive(Debug)]
pub enum MultimediaController {
    MultimediaVideoController,
    MultimediaAudioController,
    ComputerTelephonyDevice,
    AudioDevice,
    Other,
}

#[derive(Debug)]
pub enum MemoryController {
    RAMController,
    FlashController,
    Other,
}

#[derive(Debug)]
pub enum BridgeDevice {
    HostBridge,
    ISAbridge,
    EISAbridge,
    MCAbridge,
    PCItoPCIbridge,
    PCMCIAbridge,
    NuBusbridge,
    CardBusbridge,
    RACEwaybridge,
    PCItoPCIbridgeSemiTransparent,
    InfiniBandtoPCIHostBridge,
    Other,
}

#[derive(Debug)]
pub enum SimpleCommunicationController {
    SerialController,
    ParallelController,
    MultiportSerialController,
    Modem,
    GPIBController,
    SmardCardController,
    Other,
}

#[derive(Debug)]
pub enum BaseSystemPeripheral {
    Pic,
    DMAController,
    Timer,
    Rtc,
    PCIHotPlugController,
    SDHostController,
    Iommu,
    Other,
}

#[derive(Debug)]
pub enum InputDeviceController {
    KeyboardController,
    DigitizerPen,
    MouseController,
    ScannerController,
    GameportController,
    Other,
}

#[derive(Debug)]
pub enum Processor {
    I386,
    I486,
    Pentium,
    Alpha,
    PowerPC,
    Mips,
    CoProcessor,
    Other,
}

#[derive(Debug)]
pub enum SerialBusController {
    FireWireController,
    ACCESSBusController,
    Ssa,
    USBController,
    FibreChannelController,
    SMBus,
    InfiniBandController,
    IPMIController,
    Other,
}

#[derive(Debug)]
pub enum WirelessController {
    IRController,
    ConsumerIRController,
    RFController,
    BluetoothController,
    BroadbandController,
    EthernetController,
    Other,
}

#[derive(Debug)]
pub enum SatelliteCommunicationController {
    TVController,
    AudioController,
    VoiceController,
    DataController,
}

#[derive(Debug)]
pub enum EncryptionController {
    NetworkAndComputingEncryptionDevice,
    EntertainmentEncryptionDevice,
    Other,
}

#[derive(Debug)]
pub enum SignalProcessingController {
    DPIOmodule,
    PerformanceCounters,
    Other,
}
