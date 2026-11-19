//linux __sifields union

use core::fmt::Debug;

use crate::{memory::addresses::VirtAddr, proc::signal::SignalFields};

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) union LinSiSigval {
    sival_int: i32,
    sival_ptr: VirtAddr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiKillData {
    pid: u32,
    uid: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiTimerData {
    timer_id: u32,
    overrun_count: u32,
    sigval: LinSiSigval,
    sys_private: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiRtData {
    pid: u32,
    uid: u32,
    sigval: LinSiSigval,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigchldData {
    pid: u32,
    uid: u32,
    status: u32,
    utime: u64,
    stime: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigFaultAddrBnd {
    dummy_bnd: [u8; core::mem::align_of::<*const u8>()],
    lower_bound: VirtAddr,
    upper_bound: VirtAddr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigFaultAddrPkey {
    dummy_pkey: [u8; core::mem::align_of::<*const u8>()],
    pkey: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigFaultPerf {
    data: u64,
    type_: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) union LinSiSigFaultInner {
    trap_no: i32,
    addr_lsb: i16,
    addr_bnd: LinSiSigFaultAddrBnd,
    addr_pkey: LinSiSigFaultAddrPkey,
    perf: LinSiSigFaultPerf,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigfaultData {
    addr: VirtAddr,
    inner: LinSiSigFaultInner,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigpollData {
    band: i64,
    fd: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LinSiSigsysData {
    call_addr: VirtAddr,
    syscall: i32,
    arch: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) union LinSiFields {
    kill: LinSiKillData,
    timer: LinSiTimerData,
    rt: LinSiRtData,
    sigchld: LinSiSigchldData,
    sigfault: LinSiSigfaultData,
    sigpoll: LinSiSigpollData,
    sigsys: LinSiSigsysData,
}

impl Default for LinSiFields {
    fn default() -> Self {
        Self {
            kill: LinSiKillData { pid: 0, uid: 0 },
        }
    }
}

impl SignalFields for LinSiFields {
    fn get_data_as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        let ptr = self as *const Self as *const u8;
        unsafe { core::slice::from_raw_parts(ptr, size) }
    }
}

impl Debug for LinSiFields {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LinSiFields")
            .field("bytes", &self.get_data_as_bytes())
            .finish()
    }
}
