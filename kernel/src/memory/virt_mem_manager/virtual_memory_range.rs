use bitfield::bitfield;
use core::{ops::Range, sync::atomic::AtomicU32};
use std::{error::KernelError, kerror, lock_w_info, sync::no_int_spinlock::NoIntSpinlock};

use crate::memory::{self, addresses::*, physical_allocator, virt_mem_manager::page_table::PageTable};

#[derive(Debug, Clone, Copy)]
pub enum VirtualMemoryRangeCapacity {
    _4KB = 0,
    _2MB = 1,
    _1GB = 2,
    _05TB = 3,
}

impl VirtualMemoryRangeCapacity {
    pub fn from_level(level: u8) -> Option<Self> {
        let capacity = match level {
            0 => VirtualMemoryRangeCapacity::_4KB,
            1 => VirtualMemoryRangeCapacity::_2MB,
            2 => VirtualMemoryRangeCapacity::_1GB,
            3 => VirtualMemoryRangeCapacity::_05TB,
            _ => return None,
        };
        Some(capacity)
    }

    pub fn into_level(self) -> u8 {
        match self {
            VirtualMemoryRangeCapacity::_4KB => 0,
            VirtualMemoryRangeCapacity::_2MB => 1,
            VirtualMemoryRangeCapacity::_1GB => 2,
            VirtualMemoryRangeCapacity::_05TB => 3,
        }
    }

    pub fn pages(self) -> u32 {
        let level = self.into_level();
        512u64.pow(level as u32) as u32
    }

    pub fn reserved_range(&self, start: VirtAddr) -> Range<VirtAddr> {
        let level = self.into_level();
        let size = 4096 * 512u64.pow(level as u32);
        start..(start + size)
    }

    pub fn align_up(&self, addr: VirtAddr) -> VirtAddr {
        let level = self.into_level();
        let size = 4096 * 512u64.pow(level as u32);
        VirtAddr((addr.0 + size - 1) & !(size - 1))
    }

    pub fn align_down(&self, addr: VirtAddr) -> VirtAddr {
        let level = self.into_level();
        let size = 4096 * 512u64.pow(level as u32);
        VirtAddr(addr.0 & !(size - 1))
    }
}

bitfield! {
    pub struct VirtualMemoryRangePermissions(u8);
    impl Debug;
    pub write, set_write: 0;
    pub execute, set_execute: 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualMemoryRangeManagementMode {
    Managed(VirtualMemoryRangeGrowDirection),
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualMemoryRangeGrowDirection {
    Up,
    Down,
}

#[derive(Debug)]
pub struct VirtualMemoryRange {
    virt_tree_node: PhysAddr,
    virt_tree_level: u8, //0 means just 1 page, 1 means page tree node with allocated pages below
    mem_range_type: VirtualMemoryRangeManagementMode,
    perms: VirtualMemoryRangePermissions,
    allocated_pages: AtomicU32,
    alloc_lock: NoIntSpinlock<()>,
}

impl VirtualMemoryRange {
    pub fn max_size(&self) -> VirtualMemoryRangeCapacity {
        VirtualMemoryRangeCapacity::from_level(self.virt_tree_level).expect("Invalid virt tree level in VirtualMemoryRange")
    }

    pub fn current_size_pages(&self) -> u32 {
        self.allocated_pages.load(core::sync::atomic::Ordering::Relaxed)
    }

    fn set_current_size_pages(&self, new_size: u32) {
        self.allocated_pages.store(new_size, core::sync::atomic::Ordering::Relaxed);
    }

    pub fn level(&self) -> u8 {
        self.virt_tree_level
    }

    pub fn reserved_range(&self, start_addr: VirtAddr) -> Range<VirtAddr> {
        self.max_size().reserved_range(start_addr)
    }

    pub fn node_addr(&self) -> PhysAddr {
        self.virt_tree_node
    }

    pub fn permissions(&self) -> VirtualMemoryRangePermissions {
        self.perms
    }

    pub fn create(
        capacity: VirtualMemoryRangeCapacity,
        perms: VirtualMemoryRangePermissions,
        mem_range_management_mode: VirtualMemoryRangeManagementMode,
    ) -> Self {
        let owned_table_addr = physical_allocator::allocate();
        let table_addr = owned_table_addr.0;
        core::mem::forget(owned_table_addr); // Deallocation handled by drop impl
        let table = unsafe { get_at_addr::<PageTable, _>(table_addr) };
        table.clear();

        Self {
            virt_tree_node: table_addr,
            virt_tree_level: capacity.into_level(),
            allocated_pages: AtomicU32::new(0),
            perms,
            mem_range_type: mem_range_management_mode,
            alloc_lock: NoIntSpinlock::new(()),
        }
    }

    pub fn expand_by(&self, n_pages: u32) -> Result<(), KernelError> {
        let current_pages = self.current_size_pages();
        let new_size = current_pages.saturating_add(n_pages);
        self.expand_to(new_size)
    }

    pub fn expand_to(&self, n_pages: u32) -> Result<(), KernelError> {
        let VirtualMemoryRangeManagementMode::Managed(grow_direction) = self.mem_range_type else {
            return kerror!(InvalidOperation);
        };
        if n_pages > self.max_size().pages() {
            return kerror!(InvalidArgument);
        }
        if n_pages <= self.current_size_pages() {
            return kerror!(InvalidArgument);
        }

        let range = if grow_direction == VirtualMemoryRangeGrowDirection::Up {
            self.current_size_pages()..n_pages
        } else {
            let max = self.max_size().pages();
            let start = max.saturating_sub(self.current_size_pages());
            let new_start = max.saturating_sub(n_pages);
            new_start..start
        };

        self.allocate_manual(range)?;
        Ok(())
    }

    pub fn shrink_by(&mut self, n_pages: u32) -> Result<(), KernelError> {
        let current_pages = self.current_size_pages();
        let new_size = current_pages.saturating_sub(n_pages);
        self.shrink_to(new_size)
    }

    pub fn shrink_to(&self, n_pages: u32) -> Result<(), KernelError> {
        let VirtualMemoryRangeManagementMode::Managed(grow_direction) = self.mem_range_type else {
            return kerror!(InvalidOperation);
        };

        let range = if grow_direction == VirtualMemoryRangeGrowDirection::Up {
            n_pages..self.current_size_pages()
        } else {
            let max = self.max_size().pages();
            let start = max.saturating_sub(self.current_size_pages());
            let new_start = max.saturating_sub(n_pages);
            start..new_start
        };

        self.free_manual(range)
    }

    pub fn allocate_manual_external(&self, pages_to_map: Range<u32>) -> Result<(), KernelError> {
        if self.mem_range_type != VirtualMemoryRangeManagementMode::Manual {
            return kerror!(InvalidOperation);
        }
        self.allocate_manual(pages_to_map)?;
        Ok(())
    }

    fn allocate_manual(&self, pages_to_map: Range<u32>) -> Result<(), KernelError> {
        let alloc_lock = lock_w_info!(self.alloc_lock);
        let newly_allocated_pages = pages_to_map.end - pages_to_map.start;
        let current_allocated = self.current_size_pages();

        memory::userspace_map(pages_to_map, self.perms, self.virt_tree_node, self.virt_tree_level, 0)?;

        self.set_current_size_pages(current_allocated.saturating_add(newly_allocated_pages));

        drop(alloc_lock);
        Ok(())
    }

    pub fn free_manual_external(&self, pages_to_free: Range<u32>) -> Result<(), KernelError> {
        if self.mem_range_type != VirtualMemoryRangeManagementMode::Manual {
            return kerror!(InvalidOperation);
        }
        self.free_manual(pages_to_free)?;
        Ok(())
    }

    fn free_manual(&self, pages_to_free: Range<u32>) -> Result<(), KernelError> {
        let alloc_lock = lock_w_info!(self.alloc_lock);
        let freed_pages = pages_to_free.end - pages_to_free.start;
        let current_allocated = self.current_size_pages();

        memory::userspace_unmap(pages_to_free, self.virt_tree_node, self.virt_tree_level, 0)?;

        self.set_current_size_pages(current_allocated.saturating_sub(freed_pages));

        drop(alloc_lock);
        Ok(())
    }
}

impl Drop for VirtualMemoryRange {
    fn drop(&mut self) {
        //physical deallocated by dropping the phys_ranges
        PageTable::delete(self.virt_tree_node, self.virt_tree_level, false);
    }
}

impl VirtualMemoryRangeManagementMode {
    pub fn from_u64(value: u64) -> Option<Self> {
        let lower = value & 0xFFFF_FFFF;
        let upper = value >> 32;
        let management_mode = match upper {
            0 => {
                let grow_direction = match lower {
                    0 => VirtualMemoryRangeGrowDirection::Up,
                    1 => VirtualMemoryRangeGrowDirection::Down,
                    _ => return None,
                };
                VirtualMemoryRangeManagementMode::Managed(grow_direction)
            }
            1 => VirtualMemoryRangeManagementMode::Manual,
            _ => return None,
        };
        Some(management_mode)
    }
}
