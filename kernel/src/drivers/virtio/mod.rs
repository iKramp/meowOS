//!Common structs and functions for all virtio devices

mod virtqueue;

use bitfield::bitfield;

bitfield! {
    pub(super) struct DeviceStatus(u8);
    impl Debug;
    acknowledge, set_acknowledge: 0;
    driver, set_driver: 1;
    driver_ok, set_driver_ok: 2;
    features_ok, set_features_ok: 3;
    device_needs_reset, set_device_needs_reset: 6;
    failed, set_failed: 7;
}

pub(super) fn ack_virtio_device(status: &mut DeviceStatus) {
    status.set_acknowledge(true);
    //only a valid virtio driver will trigger ack
    status.set_driver(true);
}
