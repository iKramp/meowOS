pub mod block_device;
pub mod filesystem;
pub mod gpt;
pub mod net_device;
pub mod pci;
pub mod ps2;

pub fn init_drivers() {
    block_device::init_drivers();
    net_device::init_drivers();
}
