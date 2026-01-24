pub mod block_device;
pub mod gpt;
pub mod filesystem;
pub mod pci;
pub mod net_device;

pub fn init_drivers() {
    block_device::init_drivers();
    net_device::init_drivers();
}
