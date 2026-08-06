use core::error::Error;

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelErrorCode {
    Unknown,
    InodeNotPresent,
    InvalidString,
    FileSystemInconsistency,
    InternalFSError,
    NotMounted,
    NoEntry,
    UnsupportedFilesystem,
    InsufficientPermissions,
    UnsupportedOperation,
    InvalidPointer,
    AlreadyMapped,
    NotMapped,
    InsufficientResources,
    InvalidArgument,
    InvalidOperation,
    Timeout,
    IllegalValue,
    InvalidProcessFile,
    OutOfMemory,
}

#[derive(Debug)]
pub struct KernelError {
    pub code: KernelErrorCode,
    pub line: u32,
    pub file: &'static str,
}

#[macro_export]
macro_rules! kerror {
    ($code:ident) => {
        Err(KernelError {
            code: $crate::error::KernelErrorCode::$code,
            line: line!(),
            file: file!(),
        })
    };
}

#[macro_export]
macro_rules! kerror_unwrapped {
    ($code:ident) => {
        KernelError {
            code: $crate::error::KernelErrorCode::$code,
            line: line!(),
            file: file!(),
        }
    };
}

impl Error for KernelError {}

impl core::fmt::Display for KernelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Kernel error: {} at {}:{}", self.code, self.file, self.line)
    }
}

impl core::fmt::Display for KernelErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KernelErrorCode::Unknown => write!(f, "Unknown error"),
            KernelErrorCode::InodeNotPresent => write!(f, "Inode not present"),
            KernelErrorCode::InvalidString => write!(f, "Invalid string"),
            KernelErrorCode::FileSystemInconsistency => write!(f, "File system inconsistency"),
            KernelErrorCode::NotMounted => write!(f, "No mountpoint at this inode, or this dev is not mounted"),
            KernelErrorCode::InternalFSError => write!(f, "Internal file system error"),
            KernelErrorCode::NoEntry => write!(f, "No entry (usually in a map, like filesystem, partition,...)"),
            KernelErrorCode::UnsupportedFilesystem => write!(f, "Filesystem type is unsupported"),
            KernelErrorCode::InsufficientPermissions => write!(f, "Insufficient permissions"),
            KernelErrorCode::UnsupportedOperation => write!(f, "Unsupported operation"),
            KernelErrorCode::InvalidPointer => write!(f, "invalid poitner"),
            KernelErrorCode::AlreadyMapped => write!(f, "requested virtual address in mmap is already mapped"),
            KernelErrorCode::NotMapped => write!(f, "some address had to be mapped, but wasn't"),
            KernelErrorCode::InsufficientResources => write!(f, "No resources (OOM, storage,...)"),
            KernelErrorCode::InvalidArgument => write!(f, "Invalid argument"),
            KernelErrorCode::InvalidOperation => write!(f, "Invalid operation in the current state"),
            KernelErrorCode::Timeout => write!(f, "Operation timed out"),
            KernelErrorCode::IllegalValue => write!(f, "Illegal value"),
            KernelErrorCode::InvalidProcessFile => write!(f, "Invalid process file (not valid ELF)"),
            KernelErrorCode::OutOfMemory => write!(f, "Out of memory"),
        }
    }
}
