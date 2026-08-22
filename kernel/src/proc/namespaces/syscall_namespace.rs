use std::{
    boxed::Box,
    error::KernelError,
    kerror, kerror_unwrapped, lock_w_info, println,
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

    fn create_empty(id: u64) -> Result<Self, KernelError> {
        Ok(Self {
            id,
            mapped_syscalls: NoIntSpinlock::new(Vec::new()),
        })
    }

    fn create_from(id: u64, other: &Self) -> Result<Self, KernelError> {
        let new_namespace = Self::create_empty(id)?;
        let other_mapped_syscalls = lock_w_info!(other.mapped_syscalls);
        let mut mapped_syscalls = lock_w_info!(new_namespace.mapped_syscalls);
        mapped_syscalls.clear();
        for mapped in other_mapped_syscalls.iter() {
            mapped_syscalls.push(MappedSyscallPack {
                base: mapped.base,
                mask: mapped.mask,
                pack_id: mapped.pack_id,
                pack: mapped.pack.clone(),
            });
        }
        drop(mapped_syscalls);
        Ok(new_namespace)
    }

    fn get_default(holder: &super::ProcNamespaces) -> Arc<Self> {
        holder.syscall_namespace.clone()
    }
}

impl SyscallNamespace {
    pub fn default(id: u64) -> Self {
        let ns = Self::create_empty(id).expect("can't fail to create empty syscall namespace");

        let (legacy_pack, pack_id) = syscall::get_syscall_pack("legacy").expect("legacy syscall pack not found");
        ns.map_syscall_pack(0, legacy_pack, pack_id).expect("args should be valid");

        let (syscall_pack, pack_id) = syscall::get_syscall_pack("syscall_management").expect("syscall pack not found");
        ns.map_syscall_pack(0xFFFFFFE0, syscall_pack, pack_id)
            .expect("args should be valid");

        ns
    }

    pub fn map_syscall_pack(&self, base_index: u32, pack: Arc<SyscallPack>, pack_id: u64) -> Result<(), KernelError> {
        if base_index > u32::MAX - 31 {
            return kerror!(InvalidArgument);
        }

        if base_index % 32 != 0 {
            return kerror!(InvalidArgument);
        }

        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);

        let Err(pos) = mapped_syscalls.binary_search_by(|e| e.base.cmp(&base_index)) else {
            //already mapped
            return kerror!(InvalidArgument);
        };

        mapped_syscalls.insert(
            pos,
            MappedSyscallPack {
                base: base_index,
                mask: u32::MAX,
                pack,
                pack_id,
            },
        );

        Ok(())
    }

    pub fn unmap_syscall_pack_by_offset(&self, offset: u64) -> Result<(), KernelError> {
        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pos = mapped_syscalls.binary_search_by(|m| m.pack_id.cmp(&offset));
        match pos {
            Ok(pos) => {
                mapped_syscalls.remove(pos);
                Ok(())
            }
            Err(_) => kerror!(InvalidArgument),
        }
    }

    pub fn disable_syscall(&self, syscall_number: u32) -> Result<(), KernelError> {
        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pack = Self::get_pack_mut(syscall_number, &mut mapped_syscalls).ok_or(kerror_unwrapped!(InvalidArgument))?;
        let in_pack_index = syscall_number
            .checked_sub(pack.base)
            .ok_or(kerror_unwrapped!(InvalidArgument))?;
        pack.mask |= !(1 << in_pack_index);
        Ok(())
    }

    pub fn disable_syscall_by_mask(&self, pack_offset: u32, mask: u32) -> Result<(), KernelError> {
        let mut mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pack = mapped_syscalls
            .iter_mut()
            .find(|m| m.base == pack_offset)
            .ok_or(kerror_unwrapped!(InvalidArgument))?;
        pack.mask |= mask;
        Ok(())
    }

    pub fn get_syscall_handler(&self, syscall_number: u32) -> Option<SyscallHandler> {
        let mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        let pack = Self::get_pack(syscall_number, &mapped_syscalls)?;
        let in_pack_index = syscall_number.checked_sub(pack.base)?;

        println!(
            "syscall_number: {syscall_number:X}, pack.base: {:X}, in_pack_index: {:X}",
            pack.base, in_pack_index
        );

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
        let found = mapped_syscalls.get(pos)?;
        if syscall_number < found.base || syscall_number as u64 >= found.base as u64 + 32 {
            return None;
        }
        Some(found)
    }

    fn get_pack_mut(syscall_number: u32, mapped_syscalls: &mut [MappedSyscallPack]) -> Option<&mut MappedSyscallPack> {
        let pos = mapped_syscalls.binary_search_by(|e| e.base.cmp(&syscall_number));
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => pos.saturating_sub(1), //err returns pos where it could be inserted
        };
        let found = mapped_syscalls.get_mut(pos)?;
        if syscall_number < found.base || syscall_number as u64 >= found.base as u64 + 32 {
            return None;
        }
        Some(found)
    }

    /// Returns a list of (base, mask, pack_id) for all mapped syscall packs
    pub fn get_mapped_syscalls(&self) -> Box<[(u32, u32, u64)]> {
        let mapped_syscalls = lock_w_info!(self.mapped_syscalls);
        mapped_syscalls.iter().map(|m| (m.base, m.mask, m.pack_id)).collect()
    }
}
