pub use core::arch::asm;

static mut MAX_LEAF: Option<u64> = None;

pub struct CpuidLeaf {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

#[inline]
fn check_for_overflow(leaf: u64) -> bool {
    unsafe {
        if MAX_LEAF.is_none() {
            let max_leaf;
            asm!(
                "push rbx",
                "mov eax, 0",
                "cpuid",
                "pop rbx",
                out("eax") max_leaf,
                out("ecx") _,
                out("edx") _,
            );
            MAX_LEAF = Some(max_leaf);
        }
        if let Some(max) = MAX_LEAF {
            leaf <= max
        } else {
            unreachable!("MAX_LEAF should be initialized before this check")
        }
    }
}

pub fn get_cpuid_leaf(leaf: u64) -> Option<CpuidLeaf> {
    if leaf < 0x80000000 && !check_for_overflow(leaf) {
        return None;
    }
    let eax: u32;
    let ebx: u64;
    let ecx: u32;
    let edx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov r10, rbx",
            "pop rbx",
            in("eax") leaf,
            lateout("eax") eax,
            out("r10") ebx,
            out("ecx") ecx,
            out("edx") edx,
        );
    }
    let ebx = ebx & (u32::MAX as u64);
    Some(CpuidLeaf {
        eax,
        ebx: ebx as u32,
        ecx,
        edx,
    })
}

pub fn get_manufacturer_id() -> [u8; 12] {
    let leaf = get_cpuid_leaf(0).expect("CPUID leaf 0 should always succeed");
    unsafe { std::mem::transmute((leaf.ebx, leaf.edx, leaf.ecx)) }
}
