#![allow(clippy::enum_variant_names)]
#![allow(clippy::needless_range_loop)]

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum Unclassified {
    NonVgaCompatibleDevice,
    VgaCompatibleDevice,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum DisplayController {
    VGACompatibleController,
    XGAController,
    ThreeDController,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum MultimediaController {
    MultimediaVideoController,
    MultimediaAudioController,
    ComputerTelephonyDevice,
    AudioDevice,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum MemoryController {
    RAMController,
    FlashController,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum SimpleCommunicationController {
    SerialController,
    ParallelController,
    MultiportSerialController,
    Modem,
    GPIBController,
    SmardCardController,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum InputDeviceController {
    KeyboardController,
    DigitizerPen,
    MouseController,
    ScannerController,
    GameportController,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
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

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum WirelessController {
    IRController,
    ConsumerIRController,
    RFController,
    BluetoothController,
    BroadbandController,
    EthernetController,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum SatelliteCommunicationController {
    TVController,
    AudioController,
    VoiceController,
    DataController,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum EncryptionController {
    NetworkAndComputingEncryptionDevice,
    EntertainmentEncryptionDevice,
    Other,
}

#[derive(Debug, PartialEq, Eq, Clone, PartialOrd, Ord)]
pub enum SignalProcessingController {
    DPIOmodule,
    PerformanceCounters,
    Other,
}
