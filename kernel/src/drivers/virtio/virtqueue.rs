//using plsit virtqueues

use std::alloc::boxed::Box;

const USED_RING_PADDING: usize = const {
    let used_ring_alignment: usize = 4;
    let size_before = core::mem::size_of::<VirtqAvailable>();
    used_ring_alignment - (size_before % used_ring_alignment)
};

struct DriverQueueData<const QUEUE_SIZE: u16>
where
    [(); QUEUE_SIZE as usize]: Sized,
{
    virt_queue: Box<Virtq<{ QUEUE_SIZE as usize }>>,
    descriptor_usage: [bool; QUEUE_SIZE as usize],
}

impl<const QUEUE_SIZE: u16> DriverQueueData<QUEUE_SIZE>
where
    [(); QUEUE_SIZE as usize]: Sized,
{
    fn new() -> Self {
        let virt_queue = Box::new(Virtq::<{ QUEUE_SIZE as usize }>::new());

        DriverQueueData {
            virt_queue,
            descriptor_usage: [false; QUEUE_SIZE as usize],
        }
    }
}

#[repr(C)]
struct Virtq<const QUEUE_SIZE: usize> {
    descriptors: [VirtqDesc; QUEUE_SIZE],
    available: VirtqAvailable<QUEUE_SIZE>,
    used: VirtqUsed<QUEUE_SIZE>,
}

impl<const QUEUE_SIZE: usize> Virtq<QUEUE_SIZE> {
    fn new() -> Self {
        Virtq {
            descriptors: [VirtqDesc {
                addr: 0,
                len: 0,
                flags: 0,
                next: 0,
            }; QUEUE_SIZE],
            available: VirtqAvailable::new(),
            used: VirtqUsed {
                flags: 0,
                idx: 0,
                ring: [VirtqUsedElem { id: 0, len: 0 }; QUEUE_SIZE],
                avail_event: 0,
            },
        }
    }
}

const _: () = const {
    assert!(core::mem::offset_of!(Virtq, used) % 4 == 0, "used not aligned to 4 bytes");
};

#[repr(C)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct IndirectDescTable<const QUEUE_SIZE: usize> {
    desc: [VirtqDesc; QUEUE_SIZE],
}

#[repr(C)]
struct VirtqAvailable<const QUEUE_SIZE: usize> {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
    used_event: u16, //only used if VIRTIO_F_EVENT_IDX
}

#[repr(C)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C)]
struct VirtqUsed<const QUEUE_SIZE: usize> {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; QUEUE_SIZE],
    avail_event: u16, //only used if VIRTIO_F_EVENT_IDX
}

impl<const QUEUE_SIZE: usize> VirtqAvailable<QUEUE_SIZE> {
    fn new() -> Self {
        VirtqAvailable {
            flags: 0,
            idx: 0,
            ring: [0; QUEUE_SIZE],
            used_event: 0,
        }
    }

    // fn add_descriptor(&mut self, desc_index: u16) {
    //     let ring_index = (self.idx) % QUEUE_SIZE;
    //     self.ring[ring_index as usize] = desc_index;
    //     self.idx = (self.idx + 1) % QUEUE_SIZE;
    //     //mem barrier
    //     core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
    // }
}
