use core::fmt::Debug;
use std::{
    alloc::vec::Vec,
    r_lock_w_info,
    sync::{
        arc::Arc,
        once_lock::OnceLock,
        rw_lock::{RWLockModeRead, RWSpinlock, RWSpinlockGuard},
    },
    w_lock_w_info,
};

use crate::{
    proc::{get_namespace_id, namespaces::ProcNamespace},
    vfs::InodeIdentifier,
};

#[derive(Debug)]
pub struct MountNamespace {
    id: u64,
    mount_points: RWSpinlock<Vec<(InodeIdentifier, InodeIdentifier)>>,
    hidden_mount_points: RWSpinlock<Vec<(InodeIdentifier, InodeIdentifier)>>,
}

static KERNEL_MOUNT_NAMESPACE: OnceLock<Arc<MountNamespace>> = OnceLock::new();

pub fn get_kernel_mount_namespace() -> Arc<MountNamespace> {
    KERNEL_MOUNT_NAMESPACE
        .get_or_init(|| {
            Arc::new(MountNamespace {
                id: get_namespace_id(),
                mount_points: RWSpinlock::new(Vec::new()),
                hidden_mount_points: RWSpinlock::new(Vec::new()),
            })
        })
        .clone()
}

impl ProcNamespace for MountNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn create_from(id: u64, other: &Self) -> Result<Self, std::error::KernelError> {
        let mount_points = r_lock_w_info!(other.mount_points).clone();
        let hidden_mount_points = r_lock_w_info!(other.hidden_mount_points).clone();
        Ok(MountNamespace {
            id,
            mount_points: RWSpinlock::new(mount_points),
            hidden_mount_points: RWSpinlock::new(hidden_mount_points),
        })
    }

    fn create_empty(id: u64, other: &Self) -> Result<Self, std::error::KernelError> {
        //copy hidden mounts
        let hidden_mount_points = r_lock_w_info!(other.hidden_mount_points).clone();
        Ok(MountNamespace {
            id,
            mount_points: RWSpinlock::new(Vec::new()),
            hidden_mount_points: RWSpinlock::new(hidden_mount_points),
        })
    }

    fn get_default(holder: &super::ProcNamespaces) -> std::sync::arc::Arc<Self> {
        holder.mount_namespace.clone()
    }
}

type MpLockGuard<'a> = RWSpinlockGuard<'a, Vec<(InodeIdentifier, InodeIdentifier)>, RWLockModeRead>;

impl MountNamespace {
    pub fn add_mount_point(&self, parent: InodeIdentifier, child: InodeIdentifier, hidden: bool) {
        if hidden {
            let mut hidden_mount_points = w_lock_w_info!(self.hidden_mount_points);
            hidden_mount_points.push((parent, child));
        } else {
            let mut mount_points = w_lock_w_info!(self.mount_points);
            mount_points.push((parent, child));
        }
    }

    pub fn remove_mount_point(&self, parent: InodeIdentifier) {
        let mut mount_points = w_lock_w_info!(self.mount_points);
        mount_points.retain(|(parent_, _)| *parent_ != parent);
        //never remove hidden mount points
    }

    /// Safety: The caller must ensure ret.0 and ret.1 are valid at least until the last
    /// use of ret.2. The lifetimes for some reason do NOT enforce this
    pub unsafe fn get_iterator<'lock: 'iter, 'iter>(
        &'lock self,
    ) -> (MpLockGuard<'lock>, MpLockGuard<'lock>, MountPointIterator<'iter>) {
        let mount_points = r_lock_w_info!(self.mount_points);
        let hidden_mount_points = r_lock_w_info!(self.hidden_mount_points);

        let mpptr = mount_points.as_ptr();
        let mplen = mount_points.len();

        let hmpptr = hidden_mount_points.as_ptr();
        let hmplen = hidden_mount_points.len();

        let mp_slice = unsafe { std::slice::from_raw_parts(mpptr, mplen) };
        let hmp_slice = unsafe { std::slice::from_raw_parts(hmpptr, hmplen) };

        let iter = MountPointIterator {
            mount_points: mp_slice,
            hidden_mount_points: hmp_slice,
            index: 0,
        };
        (mount_points, hidden_mount_points, iter)
    }
}

#[derive(Clone)]
pub struct MountPointIterator<'a> {
    mount_points: &'a [(InodeIdentifier, InodeIdentifier)],
    hidden_mount_points: &'a [(InodeIdentifier, InodeIdentifier)],
    index: usize,
}

impl Debug for MountPointIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountPointIterator").field("index", &self.index).finish()
    }
}

impl MountPointIterator<'_> {
    fn new<'a>(
        mount_points: &'a [(InodeIdentifier, InodeIdentifier)],
        hidden_mount_points: &'a [(InodeIdentifier, InodeIdentifier)],
    ) -> MountPointIterator<'a> {
        MountPointIterator {
            mount_points,
            hidden_mount_points,
            index: 0,
        }
    }
}

impl Iterator for MountPointIterator<'_> {
    type Item = (InodeIdentifier, InodeIdentifier);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.mount_points.len() {
            let item = self.mount_points[self.index];
            self.index += 1;
            Some(item)
        } else if self.index < self.mount_points.len() + self.hidden_mount_points.len() {
            let item = self.hidden_mount_points[self.index - self.mount_points.len()];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
