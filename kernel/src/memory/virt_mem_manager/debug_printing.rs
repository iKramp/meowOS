use core::fmt::Display;
use std::{println, string::String};

use crate::{
    arch::x86_64::memory::current_root,
    memory::{MemoryCachingStrategy, addresses::*, virt_mem_manager::PageTable},
};

pub(in crate::memory) fn print_mem_mapping(root: Option<(PhysAddr, u64)>) {
    let (top_level_table_phys, level) = root.unwrap_or((current_root(), 4));

    if level == 0 {
        return;
    }

    let top_level_table = unsafe { get_at_addr(top_level_table_phys) };
    println!(
        "Memory mapping for root page table at physical address: {:#x?}, level {}",
        top_level_table_phys, level
    );
    if let Some(range) = print_range(top_level_table, None, level, VirtAddr(0), true, true, true) {
        println!("{range}");
    }
}

fn print_range(
    table: &PageTable,
    mut current_range: Option<MapRange>,
    level: u64,
    mut self_virt_addr: VirtAddr,
    perm_write: bool, //these come from the parent tables
    perm_user: bool,
    perm_exec: bool,
) -> Option<MapRange> {
    println!(
        "Printing range for table at virtual address: {:#x?}, level {}, perm_write: {}, perm_user: {}, perm_exec: {}",
        self_virt_addr, level, perm_write, perm_user, perm_exec
    );
    for entry in table.entries() {
        if !entry.present() {
            if let Some(range) = &current_range {
                println!("{range}");
                current_range = None;
            }
            self_virt_addr += 1_u64 << (3 + level * 9);
            continue;
        }
        if level == 1 || entry.huge_page(level as u8) {
            #[allow(clippy::collapsible_if)] //is clearer
            if let Some(curr_range) = current_range.clone() {
                let entry_write = perm_write && entry.writeable();
                let entry_user = perm_user && entry.user_accessible();
                let entry_exec = perm_exec && entry.executable();

                if curr_range.pat != entry.caching_strategy(level as u8)
                    || curr_range.write != entry_write
                    || curr_range.execute == entry_exec
                    || curr_range.user != entry_user
                    || (curr_range.phys.0 + curr_range.len != entry.address(level as u8).0
                        && curr_range.phys.0 - 0x1000 != entry.address(level as u8).0)
                {
                    println!("{curr_range}");
                    current_range = None
                }
            }

            if let Some(curr_range) = current_range.clone() {
                let new_range = MapRange {
                    len: curr_range.len + (1 << (3 + level * 9)),
                    phys: PhysAddr(curr_range.phys.0.min(entry.address(level as u8).0)),
                    ..curr_range
                };
                current_range = Some(new_range);
            } else {
                let new_range = MapRange {
                    virt: self_virt_addr,
                    len: 1 << (3 + level * 9),
                    phys: entry.address(level as u8),
                    pat: entry.caching_strategy(level as u8),
                    write: entry.writeable() && perm_write,
                    execute: entry.executable() && perm_exec,
                    user: entry.user_accessible() && perm_user,
                };
                current_range = Some(new_range);
            }
        } else {
            let lower_level_table = unsafe { get_at_addr::<PageTable, _>(entry.address(level as u8)) };
            let new_range = print_range(
                lower_level_table,
                current_range.clone(),
                level - 1,
                self_virt_addr,
                perm_write && entry.writeable(),
                perm_user && entry.user_accessible(),
                perm_exec && entry.executable(),
            );
            current_range = new_range;
        }
        self_virt_addr += 1_u64 << (3 + level * 9);
    }
    current_range
}

#[derive(Debug, Clone)]
struct MapRange {
    pub virt: VirtAddr,
    pub phys: PhysAddr,
    pub len: u64,
    pub pat: MemoryCachingStrategy,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
}

impl Display for MapRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut rwxu = String::from("r");
        if self.write {
            rwxu.push('w');
        } else {
            rwxu.push('-');
        }
        if self.execute {
            rwxu.push('x');
        } else {
            rwxu.push('-');
        }
        if self.user {
            rwxu.push('u');
        } else {
            rwxu.push('k');
        }
        let addr_start = if self.virt.0 & (1 << 47) != 0 {
            self.virt.0 + (0xFFFF << 48)
        } else {
            self.virt.0
        };
        let addr_end = if self.virt.0 & (1 << 47) != 0 {
            (self.virt.0 + (0xFFFF << 48)).wrapping_add(self.len)
        } else {
            self.virt.0 + self.len
        };
        write!(
            f,
            "Range: virt: {:016x}, end: {:016x}, phys start: {:016x}, pat: {:?}, rwx: {:?}",
            addr_start, addr_end, self.phys.0, self.pat, rwxu
        )
    }
}
