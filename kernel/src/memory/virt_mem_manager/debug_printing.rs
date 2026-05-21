use core::fmt::Display;
use std::{
    mem_utils::{PhysAddr, VirtAddr, get_at_physical_addr},
    println,
    string::String,
};

use crate::memory::{self, LiminePat, virt_mem_manager::PageTable};

pub(in crate::memory) fn print_mem_mapping() {
    let top_level_table_phys = memory::current_root();
    let top_level_table = unsafe { get_at_physical_addr(top_level_table_phys) };
    if let Some(range) = print_range(top_level_table, None, 4, VirtAddr(0)) {
        println!("{range}");
    }
}

fn print_range(
    table: &PageTable,
    mut current_range: Option<MapRange>,
    level: u64,
    mut self_virt_addr: VirtAddr,
) -> Option<MapRange> {
    println!(
        "entered print_range with params: level: {}, self_virt_addr: {:016x}, current_range: {:?}",
        level, self_virt_addr.0, current_range
    );
    println!("table addr: {:016x?}", table as *const PageTable as u64);

    for entry in &table.entries {
        if !entry.present() {
            if let Some(range) = &current_range {
                println!("{range}");
                current_range = None;
            }
            self_virt_addr += 1 << (3 + level * 9);
            continue;
        }
        if level == 1 || entry.huge_page() {
            #[allow(clippy::collapsible_if)] //is clearer
            if let Some(curr_range) = current_range.clone() {
                if curr_range.pat != entry.pat()
                    || curr_range.write != entry.writeable()
                    || curr_range.execute == entry.no_execute()
                    || (curr_range.phys.0 + curr_range.len != entry.address().0
                        && curr_range.phys.0 - 0x1000 != entry.address().0)
                {
                    println!("{curr_range}");
                    current_range = None
                }
            }

            if let Some(curr_range) = current_range.clone() {
                let new_range = MapRange {
                    len: curr_range.len + (1 << (3 + level * 9)),
                    phys: PhysAddr(curr_range.phys.0.min(entry.address().0)),
                    ..curr_range
                };
                current_range = Some(new_range);
            } else {
                let new_range = MapRange {
                    virt: self_virt_addr,
                    len: 1 << (3 + level * 9),
                    phys: entry.address(),
                    pat: entry.pat(),
                    write: entry.writeable(),
                    execute: !entry.no_execute(),
                    user: entry.user_accessible(),
                };
                current_range = Some(new_range);
            }
        } else {
            let lower_level_table = unsafe { get_at_physical_addr::<PageTable>(entry.address()) };
            let new_range = print_range(lower_level_table, current_range.clone(), level - 1, self_virt_addr);
            current_range = new_range;
        }
        self_virt_addr += 1 << (3 + level * 9);
    }
    current_range
}

#[derive(Debug, Clone)]
struct MapRange {
    pub virt: VirtAddr,
    pub phys: PhysAddr,
    pub len: u64,
    pub pat: LiminePat,
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
