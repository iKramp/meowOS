use std::alloc::boxed::Box;

mod disk;

pub(super) fn init_driver() {
    crate::drivers::pci::register_legacy_pci_driver(None, Some(0x1AF4), Some(0x1001), None, None, || {
        Box::new(disk::VirtioDiskDriver::new())
    });
    crate::drivers::pci::register_legacy_pci_driver(None, Some(0x1AF4), Some(0x1042), None, None, || {
        Box::new(disk::VirtioDiskDriver::new())
    });
}
