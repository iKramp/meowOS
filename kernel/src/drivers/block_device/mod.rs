pub mod ahci;
pub mod block_dev_cache;
pub mod disk;
pub mod virtio;

pub(super) fn init_drivers() {
    ahci::init_driver();
    virtio::init_driver();
}
