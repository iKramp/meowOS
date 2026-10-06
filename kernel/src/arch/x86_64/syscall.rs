use crate::proc::syscall::main_syscall_handler;

//sys V abi:
//ret val: rax, rdx
//parameters: rdi, rsi, rdx, rcx, r8, r9
//scratch regs: rax, rdi, rsi, rdx, rcx, r8, r9, r10, r11
//preserved: rbx, rsp, rbp, r12 - r15

//linux syscall abi:
//ret val: rax, rdx
//parameters: rdi, rsi, rdx, r10, r8, r9
//syscall number: rax
//x86-reserved: rcx, r11
//preserved: rbx, rbp, r12 - r15

//syscalls are limited to 5 64bit parameters. If more data is needed, set up a structure and pass a
//pointer to it
#[unsafe(naked)]
extern "C" fn syscall_entry() -> ! {
    //INFO: any kind of change here should be matched with the one in dispatcher.rs
    core::arch::naked_asm!(
        //push preserved regs, get kernel stack from gsbase
        //stack is aligned to 16 here
        "swapgs",

        "mov gs:[16], rcx", //save user rip to gsbase area
        "mov cx, 0",
        "mov ss, cx",
        "mov rcx, gs:[16]", //get user rip from gsbase area

        "mov gs:[16], rsp", //save user rsp to gsbase area
        "mov rsp, gs:[8]", //get kernel rsp from gsbase area

        "sub rsp, 8*16", //space for 16 u64s
        "mov [rsp + 0*8], rcx",
        "mov [rsp + 1*8], rbp",
        "mov rcx, gs:[16]", //user rsp
        "mov [rsp + 2*8], rcx", //user rsp
        "mov [rsp + 3*8], r11",
        "mov [rsp + 4*8], rax",
        "mov [rsp + 5*8], rbx",
        "mov [rsp + 6*8], rdx",
        "mov [rsp + 7*8], rdi",
        "mov [rsp + 8*8], rsi",
        "mov [rsp + 9*8], r8",
        "mov [rsp + 10*8], r9",
        "mov [rsp + 11*8], r10",
        "mov [rsp + 12*8], r12",
        "mov [rsp + 13*8], r13",
        "mov [rsp + 14*8], r14",
        "mov [rsp + 15*8], r15",

        "mov rdi, rsp", //args rsp

        "call {}",
        sym main_syscall_handler
    )
}

#[unsafe(naked)]
pub extern "C" fn return_syscalled(cpu_state: &SyscallCpuState) -> ! {
    //INFO: any kind of change here should be matched with the one in syscall.rs
    core::arch::naked_asm!(
        //cpu_state in rdi
        "mov rcx, [rdi + 8 * 0]",
        "mov rbp, [rdi + 8 * 1]",
        "mov rsp, [rdi + 8 * 2]",
        "mov r11, [rdi + 8 * 3]",
        "mov rax, [rdi + 8 * 4]",
        "mov rbx, [rdi + 8 * 5]",
        "mov rdx, [rdi + 8 * 6]", //user rsp
        "mov rsi,  [rdi + 8 * 8]",
        "mov r8,  [rdi + 8 * 9]",
        "mov r9, [rdi + 8 * 10]",
        "mov r10, [rdi + 8 * 11]",
        "mov r12, [rdi + 8 * 12]",
        "mov r13, [rdi + 8 * 13]",
        "mov r14, [rdi + 8 * 14]",
        "mov r15, [rdi + 8 * 15]",
        "mov rdi, [rdi + 8 * 7]",
        "swapgs", //restore gs for user code
        "sysretq",
    )
}

#[derive(Clone)]
#[repr(C)]
pub struct SyscallCpuState {
    pub rcx: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r11: u64,
    pub rax: u64, //syscall number
    pub rbx: u64, //namespace id
    pub rdx: u64, //args start here
    pub rdi: u64,
    pub rsi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl SyscallCpuState {
    pub fn get_legacy_syscall_arg(&self, index: usize) -> u64 {
        match index {
            1 => self.rdi,
            2 => self.rsi,
            3 => self.rdx,
            4 => self.r10,
            5 => self.r8,
            6 => self.r9,
            _ => panic!("Invalid legacy syscall argument index: {}", index),
        }
    }

    pub fn get_arg(&self, index: usize) -> u64 {
        match index {
            0 => self.rdx,
            1 => self.rdi,
            2 => self.rsi,
            3 => self.r8,
            4 => self.r9,
            5 => self.r10,
            6 => self.r12,
            7 => self.r13,
            8 => self.r14,
            9 => self.r15,
            _ => panic!("Invalid syscall argument index: {}", index),
        }
    }

    pub fn get_syscall_number(&self) -> u64 {
        self.rax
    }

    pub fn get_namespace_id(&self) -> u64 {
        self.rbx
    }
}

impl Debug for SyscallCpuState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyscallCpuState")
            .field("syscall_number", &self.rax)
            .field("namespace_id", &self.rbx)
            .field("arg0", &self.rdx)
            .field("arg1", &self.rdi)
            .field("arg2", &self.rsi)
            .field("arg3", &self.r8)
            .field("arg4", &self.r9)
            .field("arg5", &self.r10)
            .field("arg6", &self.r12)
            .field("arg7", &self.r13)
            .field("arg8", &self.r14)
            .field("arg9", &self.r15)
            .finish()
    }
}
