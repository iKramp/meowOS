pub(in crate::arch::x86_64) mod cpuid;

use cpuid::get_cpuid_leaf;

pub fn is_virtualized() -> bool {
    if let Some(leaf) = get_cpuid_leaf(1) {
        (leaf.ecx & (1 << 31)) != 0
    } else {
        false
    }
}

pub fn has_hardware_fpu() -> bool {
    if let Some(leaf) = get_cpuid_leaf(1) {
        (leaf.edx & (1 << 0)) != 0
    } else {
        false
    }
}

pub struct SseSupport {
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub fxsavestore: bool,
    pub xsave: bool,
    pub clflush: bool,
}

pub fn get_sse_support() -> SseSupport {
    if let Some(leaf) = get_cpuid_leaf(1) {
        SseSupport {
            sse: (leaf.edx & (1 << 25)) != 0,
            sse2: (leaf.edx & (1 << 26)) != 0,
            sse3: (leaf.ecx & (1 << 0)) != 0,
            ssse3: (leaf.ecx & (1 << 9)) != 0,
            fxsavestore: (leaf.edx & (1 << 24)) != 0,
            xsave: (leaf.ecx & (1 << 26)) != 0,
            clflush: (leaf.edx & (1 << 19)) != 0,
        }
    } else {
        SseSupport {
            sse: false,
            sse2: false,
            sse3: false,
            ssse3: false,
            fxsavestore: false,
            xsave: false,
            clflush: false,
        }
    }
}
