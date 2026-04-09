use std::{error::ErrorCode, sync::arc::Arc, vec::Vec};

use crate::proc::syscall::{self, SyscallHandler, SyscallPack};

#[derive(Debug)]
struct MappedSyscallPack {
    base: u32,
    mask: u32,
    pack: Arc<SyscallPack>,
}

#[derive(Debug)]
pub(in crate::proc) struct SyscallNamespace {
    //sorted by index
    mapped_syscalls: Vec<MappedSyscallPack>,
}

impl SyscallNamespace {
    pub fn new() -> Self {
        Self {
            mapped_syscalls: Vec::new(),
        }
    }

    pub fn default() -> Self {
        let legacy_pack = syscall::get_syscall_pack("legacy").expect("legacy syscall pack not found");
        let mut ns = Self::new();
        ns.map_syscall_pack(0, legacy_pack);
        ns
    }

    pub fn map_syscall_pack(&mut self, base_index: u32, pack: Arc<SyscallPack>) {
        self.mapped_syscalls.push(MappedSyscallPack {
            base: base_index,
            mask: u32::MAX,
            pack,
        });
    }

    pub fn disable_syscall(&mut self, syscall_number: u32) -> Result<(), ErrorCode> {
        let pack = self.get_pack_mut(syscall_number).ok_or(ErrorCode::InvalidArgument)?;
        let in_pack_index = syscall_number.checked_sub(pack.base).ok_or(ErrorCode::InvalidArgument)?;
        pack.mask |= !(1 << in_pack_index);
        Ok(())
    }

    pub fn get_syscall_handler(&self, syscall_number: u32) -> Option<SyscallHandler> {
        let pack = self.get_pack(syscall_number)?;
        let in_pack_index = syscall_number.checked_sub(pack.base)?;
        if pack.mask & (1 << in_pack_index) == 0 {
            return None;
        }
        pack.pack.get_handler(in_pack_index as usize)
    }

    fn get_pack(&self, syscall_number: u32) -> Option<&MappedSyscallPack> {
        let pos = self.mapped_syscalls.binary_search_by(|e| e.base.cmp(&syscall_number));
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => pos.saturating_sub(1), //err returns pos where it could be inserted
        };
        self.mapped_syscalls.get(pos)
    }

    fn get_pack_mut(&mut self, syscall_number: u32) -> Option<&mut MappedSyscallPack> {
        let pos = self.mapped_syscalls.binary_search_by(|e| e.base.cmp(&syscall_number));
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => pos.saturating_sub(1), //err returns pos where it could be inserted
        };
        self.mapped_syscalls.get_mut(pos)
    }
}
