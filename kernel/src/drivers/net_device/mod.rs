pub mod e1000e;

pub(super) fn init_drivers() {
    e1000e::init_driver();
}
