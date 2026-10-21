use crate::drivers::pci::{LegacyPciDriver, PcieDriver};

pub(super) struct VirtioDiskDriver;

impl VirtioDiskDriver {
    pub fn new() -> Self {
        Self
    }
}

impl LegacyPciDriver for VirtioDiskDriver {
    fn init(&mut self, dev: &crate::drivers::pci::LegacyPciDevice) -> Result<(), std::error::KernelError> {
        todo!("Virtio disk driver initialization is not implemented yet.");
    }

    fn deinit(&mut self, dev: &crate::drivers::pci::LegacyPciDevice) {
        todo!()
    }

    fn service_interrupt(&mut self, dev: &crate::drivers::pci::LegacyPciDevice) {
        todo!()
    }

    fn remove_device(&mut self) {
        todo!()
    }
}
