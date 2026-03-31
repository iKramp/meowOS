use crate::println;
use kernel_test::{kernel_test, kernel_test_mod};
use std::mem_utils;
kernel_test_mod!(crate::tests::memory_utils);

#[derive(Clone, Copy, PartialEq, Eq)]
struct ExampleStruct {
    field_1: u64,
    field_2: [u64; 4],
    field_3: bool,
}

#[kernel_test]
//this tests if our virtual to physical converter matches the hardware converter
fn virt_to_phys_addr_test() -> bool {
    let data_1 = ExampleStruct {
        field_1: 345,
        field_2: [2465, 25462, 345, 52346356736],
        field_3: true,
    };
    let root_page_table = PageTree::get_level4_addr();
    let ptr_to_data = core::ptr::addr_of!(data_1);
    let data_2 = unsafe {
        mem_utils::get_at_physical_addr::<ExampleStruct>(
            mem_utils::translate_virt_phys_addr(mem_utils::VirtAddr(ptr_to_data as u64), root_page_table)
                .unwrap_or(mem_utils::PhysAddr(0)),
        )
    };
    data_1 == *data_2
}

#[kernel_test]
fn huge_page_test() -> bool {
    unsafe {
        let root_page_table = PageTree::get_level4_addr();
        let start_addr = mem_utils::PhysAddr(6554161);
        let virtual_addr = mem_utils::translate_phys_virt_addr(start_addr); //this gets a virtual address
        //on the huge page
        let physical_addr = mem_utils::translate_virt_phys_addr(virtual_addr, root_page_table).unwrap_or(mem_utils::PhysAddr(0));

        assert_eq!(start_addr, physical_addr);
        start_addr == physical_addr
    }
}
