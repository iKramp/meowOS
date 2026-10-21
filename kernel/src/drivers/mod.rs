pub mod block_device;
pub mod filesystem;
pub mod gpt;
pub mod input;
pub mod net_device;
pub mod pci;
pub mod ps2;
mod virtio;

pub fn init_drivers() {
    block_device::init_drivers();
    net_device::init_drivers();
    filesystem::init_fs_drivers();
    ps2::init();
    input::init_input();
}
