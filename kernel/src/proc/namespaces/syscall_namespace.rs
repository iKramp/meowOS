use std::{
    boxed::Box,
    error::ErrorCode,
    lock_w_info,
    sync::{arc::Arc, no_int_spinlock::NoIntSpinlock},
    vec::Vec,
};

use crate::proc::{
    namespaces::ProcNamespace,
    syscall::{self, SyscallHandler, SyscallPack},
};

#[derive(Debug)]
struct MappedSyscallPack {
    base: u32,
    mask: u32,
    pack_id: u64,
    pack: Arc<SyscallPack>,
}

#[derive(Debug)]
pub(in crate::proc) struct SyscallNamespace {
    id: u64,
    //sorted by index
    mapped_syscalls: NoIntSpinlock<Vec<MappedSyscallPack>>,
}

impl ProcNamespace for SyscallNamespace {
    fn get_id(&self) -> u64 {
        self.id
    }
}

impl SyscallNamespace {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            mapped_syscalls: NoIntSpinlock::new(Vec::new()),
        }
    }

    pub fn default(id: u64) -> Self {
        let (legacy_pack, pack_id) = syscall::get_syscall_pack("legacy").expect("legacy syscall pack not found");
        let ns = Self::new(id);
        ns.map_syscall_pack(0, legacy_pack, pack_id);
        ns
    }

    pub fn map_syscall_pack(&self, base_index: u32, pack: Arc<SyscallPack>, pack_id: u64) {
        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);

        mapped_syscalls.push(MappedSyscallPack {
            base: base_index,
            mask: u32::MAX,
            pack,
            pack_id,
        });
    }

    pub fn disable_syscall(&self, syscall_number: u32) -> Result<(), ErrorCode> {
        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pack = Self::get_pack_mut(syscall_number, &mut mapped_syscalls).ok_or(ErrorCode::InvalidArgument)?;
        let in_pack_index = syscall_number.checked_sub(pack.base).ok_or(ErrorCode::InvalidArgument)?;
        pack.mask |= !(1 << in_pack_index);
        Ok(())
    }

    pub fn get_syscall_handler(&self, syscall_number: u32) -> Option<SyscallHandler> {
        let mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pack = Self::get_pack(syscall_number, &mapped_syscalls)?;
        let in_pack_index = syscall_number.checked_sub(pack.base)?;
        if pack.mask & (1 << in_pack_index) == 0 {
            return None;
        }
        pack.pack.get_handler(in_pack_index as usize)
    }

    fn get_pack(syscall_number: u32, mapped_syscalls: &[MappedSyscallPack]) -> Option<&MappedSyscallPack> {
        let pos = mapped_syscalls.binary_search_by(|e| e.base.cmp(&syscall_number));
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => pos.saturating_sub(1), //err returns pos where it could be inserted
        };
        mapped_syscalls.get(pos)
    }

    fn get_pack_mut(syscall_number: u32, mapped_syscalls: &mut [MappedSyscallPack]) -> Option<&mut MappedSyscallPack> {
        let pos = mapped_syscalls.binary_search_by(|e| e.base.cmp(&syscall_number));
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => pos.saturating_sub(1), //err returns pos where it could be inserted
        };
        mapped_syscalls.get_mut(pos)
    }

    /// Returns a list of (base, mask, pack_id) for all mapped syscall packs
    pub fn get_mapped_syscalls(&self) -> Box<[(u32, u32, u64)]> {
        let mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        mapped_syscalls.iter().map(|m| (m.base, m.mask, m.pack_id)).collect()
    }
}
