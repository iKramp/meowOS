use crate::cpuid;

#[repr(C, align(32))]
struct SSEArray([f32; 4]);

//Function for initializing both the boot processor and APs
pub fn cpu_init_common() {
    init_x87_fpu();
    init_sse();

    //test SSE by performing a simple vector addition
    let a: SSEArray = SSEArray([1.0, 2.0, 3.0, 4.0]);
    let b: SSEArray = SSEArray([5.0, 6.0, 7.0, 8.0]);
    let mut c: SSEArray = SSEArray([0.0, 0.0, 0.0, 0.0]);
    unsafe {
        core::arch::asm!(
            "movaps xmm0, [{a_ptr}]",
            "movaps xmm1, [{b_ptr}]",
            "addps xmm0, xmm1",
            "movaps [{c_ptr}], xmm0",
            a_ptr = in(reg) &a,
            b_ptr = in(reg) &b,
            c_ptr = in(reg) &mut c,
        );
    }
    if c.0 != [6.0, 8.0, 10.0, 12.0] {
        panic!("SSE test failed");
    }
}

fn init_x87_fpu() {
    let cpuid = cpuid::get_cpuid_leaf(1).expect("CPUID leaf 1 should be available");
    if (cpuid.edx & (1 << 0)) == 0 {
        panic!("FPU not present on this CPU");
    }

    let mut cr0: u64;
    //initialize and grab cr0
    unsafe {
        core::arch::asm!(
            "finit",
            "mov {cr0}, cr0",
            cr0 = out(reg) cr0,
        );
    }
    cr0 &= !(1 << 2); // Clear EM (Emulation) bit
    cr0 |= 1 << 1; // Set MP (Monitor coprocessor) bit
    cr0 |= 1 << 5; // Set NE (Numeric error) bit
    unsafe {
        core::arch::asm!(
            "mov cr0, {cr0}",
            cr0 = in(reg) cr0,
        );
    }
}

fn init_sse() {
    let cpuid = cpuid::get_cpuid_leaf(1).expect("CPUID leaf 1 should be available");
    let sse_supported = (cpuid.edx & (1 << 25)) != 0;
    let sse2_supported = (cpuid.edx & (1 << 26)) != 0;
    let sse3_supported = (cpuid.ecx & (1 << 0)) != 0;
    let ssse3_supported = (cpuid.ecx & (1 << 9)) != 0;
    let fxsavestore_supported = (cpuid.edx & (1 << 24)) != 0;
    let xsave_supported = (cpuid.ecx & (1 << 26)) != 0;
    let clflush_supported = (cpuid.edx & (1 << 19)) != 0;
    if !sse_supported
        || !sse2_supported
        || !sse3_supported
        || !ssse3_supported
        || !fxsavestore_supported
        || !xsave_supported
        || !clflush_supported
    {
        panic!("SSE not fully supported on this CPU");
    }

    let mut cr4: u64;
    unsafe {
        core::arch::asm!(
            "mov {cr4}, cr4",
            cr4 = out(reg) cr4,
        );
    }
    cr4 |= 1 << 9; // Set OSFXSR (Operating System Support for FXSAVE and FXRSTOR instructions) bit
    cr4 |= 1 << 10; // Set OSXMMEXCPT (OS Support for Unmasked SIMD Floating-Point Exceptions) bit
    unsafe {
        core::arch::asm!(
            "mov cr4, {cr4}",
            cr4 = in(reg) cr4,
        );
    }

    let mut mxcsr: u32 = 0;
    unsafe {
        core::arch::asm!("stmxcsr [{mxcsr_ptr}]", mxcsr_ptr = in(reg) &mut mxcsr);
    }
    mxcsr = 0x1F80; // Set MXCSR to default value
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &mxcsr,
        );
    }
}
