use std::{
    r_lock_w_info,
    sync::{arc::Arc, once_lock::OnceLock, rw_lock::RWSpinlock},
    time::{Instant, UNIX_EPOCH},
    w_lock_w_info,
};

use crate::proc::namespaces::ProcNamespace;

#[derive(Debug, Default)]
pub(in crate::proc) struct TimeNamespace {
    id: u64,
    internal: RWSpinlock<TimeNamespaceInternal>,
}

#[derive(Debug, Default)]
struct TimeNamespaceInternal {
    //epoch realtive to real epoch
    //negative values means apparent time is in the past (lower number of seconds since epoch)
    monotonic_offset_ns: i128,
    //boot time relative to real boot time. Negative values means the apparent boot time was earlier, but
    //time since boot is larger than real time since boot
    boot_time_offset_ns: i128,
}

pub(in crate::proc) static DEFAULT_TIME_NAMESPACE: OnceLock<Arc<TimeNamespace>> = OnceLock::new();

impl TimeNamespace {
    pub fn new(id: u64, monotonic_offset_ns: i128, boot_time_offset_ns: i128) -> Self {
        TimeNamespace {
            id,
            internal: RWSpinlock::new(TimeNamespaceInternal {
                monotonic_offset_ns,
                boot_time_offset_ns,
            }),
        }
    }

    pub fn get_offsets(&self) -> (i128, i128) {
        let internal = r_lock_w_info!(self.internal);
        (internal.monotonic_offset_ns, internal.boot_time_offset_ns)
    }

    //returns if data was applied without issues (wrong args)
    pub fn apply_offset(&self, monotonic_offset_ns: i128, boot_time_offset_ns: i128) -> bool {
        let mut internal = w_lock_w_info!(self.internal);
        let new_monotonic_offset = internal.monotonic_offset_ns + monotonic_offset_ns;
        let new_boot_time_offset = internal.boot_time_offset_ns + boot_time_offset_ns;

        let now = Instant::now();
        let apparent_mono_time = (now.duration_since(UNIX_EPOCH).as_nanos() as i128).saturating_add(new_monotonic_offset);
        let apparent_since_boot =
            (now.duration_since(crate::clocks::get_time_of_boot()).as_nanos() as i128).saturating_add(new_boot_time_offset);

        if apparent_mono_time < 0 || apparent_since_boot < 0 {
            return false;
        }
        internal.monotonic_offset_ns = new_monotonic_offset;
        internal.boot_time_offset_ns = new_boot_time_offset;
        true
    }
}

impl ProcNamespace for TimeNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn create_from(id: u64, other: &Self) -> Result<Self, std::error::KernelError> {
        let (monotonic_offset, boot_time_offset) = other.get_offsets();
        Ok(TimeNamespace {
            id,
            internal: RWSpinlock::new(TimeNamespaceInternal {
                monotonic_offset_ns: monotonic_offset,
                boot_time_offset_ns: boot_time_offset,
            }),
        })
    }

    fn create_empty(id: u64) -> Result<Self, std::error::KernelError> {
        Ok(TimeNamespace {
            id,
            ..TimeNamespace::default()
        })
    }

    fn get_default(holder: &super::ProcNamespaces) -> std::sync::arc::Arc<Self> {
        holder.time_namespace.clone()
    }
}
