pub mod ahci;
pub mod disk;

pub(super) fn init_drivers() {
    ahci::init_driver();
}
