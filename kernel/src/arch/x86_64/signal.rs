use crate::{
    arch::{
        interrupts::{USER_CODE_SEGMENT, USER_STACK_SEGMENT},
        x86_64::float::FloatState,
    },
    interrupts::{InterruptProcessorState, return_interrupted},
    memory::{addresses::VirtAddr, safe_memcpy_to_user},
    proc::{
        CpuStateType,
        signal::{SignalData, SignalFrame, SignalInfo, signal_return_fn},
    },
};

//matching linux
#[repr(C)]
pub struct SignalCpuContext {
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rdi: u64,
    rsi: u64,
    rbp: u64,
    rbx: u64,
    rdx: u64,
    rax: u64,
    rcx: u64,
    rsp: u64,
    rip: u64,
    rflags: u64,
    cs: u16,
    gs: u16, //don't use
    fs: u16, //don't use
    ss: u16,
    err: u64,
    trapno: u64,  //don't use
    oldmask: u64, //don't use
    cr2: u64,     //don't use
    float_state: FloatState,
}

impl From<CpuStateType> for SignalCpuContext {
    fn from(state: CpuStateType) -> Self {
        match state {
            CpuStateType::Interrupt(interrupt_processor_state) => {
                let int_frame = &interrupt_processor_state.interrupt_frame;
                SignalCpuContext {
                    r8: interrupt_processor_state.r8,
                    r9: interrupt_processor_state.r9,
                    r10: interrupt_processor_state.r10,
                    r11: interrupt_processor_state.r11,
                    r12: interrupt_processor_state.r12,
                    r13: interrupt_processor_state.r13,
                    r14: interrupt_processor_state.r14,
                    r15: interrupt_processor_state.r15,
                    rdi: interrupt_processor_state.rdi,
                    rsi: interrupt_processor_state.rsi,
                    rbp: interrupt_processor_state.rbp,
                    rbx: interrupt_processor_state.rbx,
                    rdx: interrupt_processor_state.rdx,
                    rax: interrupt_processor_state.rax,
                    rcx: interrupt_processor_state.rcx,
                    rsp: int_frame.rsp,
                    rip: int_frame.rip,
                    rflags: int_frame.rflags,
                    cs: int_frame.cs as u16,
                    gs: 0, //not used
                    fs: 0, //not used
                    ss: int_frame.ss as u16,
                    err: interrupt_processor_state.err_code,
                    trapno: 0,  //not used
                    oldmask: 0, //not used
                    cr2: 0,     //not used
                    float_state: FloatState {},
                }
            }
            CpuStateType::Syscall(syscall_cpu_state) => {
                SignalCpuContext {
                    r8: syscall_cpu_state.r8,
                    r9: syscall_cpu_state.r9,
                    r10: syscall_cpu_state.r10,
                    r11: syscall_cpu_state.r11,
                    r12: syscall_cpu_state.r12,
                    r13: syscall_cpu_state.r13,
                    r14: syscall_cpu_state.r14,
                    r15: syscall_cpu_state.r15,
                    rdi: syscall_cpu_state.rdi,
                    rsi: syscall_cpu_state.rsi,
                    rbp: syscall_cpu_state.rbp,
                    rbx: syscall_cpu_state.rbx,
                    rdx: syscall_cpu_state.rdx,
                    rax: syscall_cpu_state.rax,
                    rcx: syscall_cpu_state.rcx,
                    rsp: syscall_cpu_state.rsp,
                    rip: syscall_cpu_state.rcx,    //rip is in rcx
                    rflags: syscall_cpu_state.r11, //rflags is in r11
                    cs: USER_CODE_SEGMENT as u16,
                    gs: 0, //not used
                    fs: 0, //not used
                    ss: USER_STACK_SEGMENT as u16,
                    err: 0,     //not used
                    trapno: 0,  //not used
                    oldmask: 0, //not used
                    cr2: 0,     //not used
                    float_state: FloatState {},
                }
            }
            CpuStateType::None => panic!("Cannot convert CpuStateType::None to SignalCpuContext"),
        }
    }
}

impl From<SignalCpuContext> for InterruptProcessorState {
    fn from(context: SignalCpuContext) -> Self {
        InterruptProcessorState {
            r15: context.r15,
            r14: context.r14,
            r13: context.r13,
            r12: context.r12,
            r11: context.r11,
            r10: context.r10,
            r9: context.r9,
            r8: context.r8,
            rbp: context.rbp,
            rdi: context.rdi,
            rsi: context.rsi,
            rdx: context.rdx,
            rcx: context.rcx,
            rbx: context.rbx,
            rax: context.rax,
            err_code: context.err,
            interrupt_frame: crate::arch::x86_64::interrupts::InterruptFrame {
                rip: context.rip,
                cs: context.cs as u64,
                rflags: context.rflags,
                rsp: context.rsp,
                ss: context.ss as u64,
            },
        }
    }
}

pub fn get_new_stack_pointer(mut curr_rsp: u64, signal_data: &SignalData) -> u64 {
    curr_rsp &= !0xf; //align to 16 bytes
    curr_rsp -= 128; //red zone
    curr_rsp -= core::mem::size_of::<SignalFrame<()>>() as u64;
    curr_rsp -= signal_data.fields.get_data_as_bytes().len() as u64;
    curr_rsp
}

pub fn dispatch_signal(cpu_state: CpuStateType, signal_handler: VirtAddr, signal_data: SignalData) {
    let data = signal_data.fields.get_data_as_bytes();

    let frame = SignalFrame {
        retcode: VirtAddr(signal_return_fn as *const () as u64),
        user_context: cpu_state.into(),
        signal_info: SignalInfo {
            signal_number: signal_data.signal_type as u32,
            ret_error: 0,
            ret_code: 0,
            signal_fields: (), //don't fill in data
        },
    };

    let rsp = get_new_stack_pointer(frame.user_context.rsp, &signal_data);

    let res = safe_memcpy_to_user(
        rsp,
        &frame as *const SignalFrame<()> as u64,
        core::mem::size_of::<SignalFrame<()>>(),
    );
    let res2 = safe_memcpy_to_user(
        rsp + core::mem::size_of::<SignalFrame<()>>() as u64,
        data.as_ptr() as u64,
        data.len(),
    );
    if !res || !res2 {
        return;
    }

    let mut int_frame = InterruptProcessorState::new(signal_handler.0, rsp);
    int_frame.rdi = signal_data.signal_type as u64;
    int_frame.rsi = rsp + core::mem::size_of::<SignalFrame<()>>() as u64; //pointer to signal info
    int_frame.rdx = rsp + core::mem::size_of::<VirtAddr>() as u64; //pointer to user context
    return_interrupted(&int_frame);
}

pub fn handle_signal_return() {
    todo!()
}
