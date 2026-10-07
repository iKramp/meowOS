use crate::arch::x86_64::cpu_identification;

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
    let has_fpu = cpu_identification::has_hardware_fpu();
    if !has_fpu {
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
    let sse_support = cpu_identification::get_sse_support();
    if !sse_support.sse
        || !sse_support.sse2
        || !sse_support.sse3
        || !sse_support.ssse3
        || !sse_support.fxsavestore
        || !sse_support.xsave
        || !sse_support.clflush
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
