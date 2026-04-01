use core::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
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
    Timeout,
    IllegalValue,
    InvalidProcessFile,
    OutOfMemory,
}

impl Error for ErrorCode {}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErrorCode::Unknown => write!(f, "Unknown error"),
            ErrorCode::InodeNotPresent => write!(f, "Inode not present"),
            ErrorCode::InvalidString => write!(f, "Invalid string"),
            ErrorCode::FileSystemInconsistency => write!(f, "File system inconsistency"),
            ErrorCode::NotMounted => write!(f, "No mountpoint at this inode, or this dev is not mounted"),
            ErrorCode::InternalFSError => write!(f, "Internal file system error"),
            ErrorCode::NoEntry => write!(f, "No entry (usually in a map, like filesystem, partition,...)"),
            ErrorCode::UnsupportedFilesystem => write!(f, "Filesystem type is unsupported"),
            ErrorCode::InsufficientPermissions => write!(f, "Insufficient permissions"),
            ErrorCode::UnsupportedOperation => write!(f, "Unsupported operation"),
            ErrorCode::InvalidPointer => write!(f, "invalid poitner"),
            ErrorCode::AlreadyMapped => write!(f, "requested virtual address in mmap is already mapped"),
            ErrorCode::NotMapped => write!(f, "some address had to be mapped, but wasn't"),
            ErrorCode::InsufficientResources => write!(f, "No resources (OOM, storage,...)"),
            ErrorCode::InvalidArgument => write!(f, "Invalid argument"),
            ErrorCode::Timeout => write!(f, "Operation timed out"),
            ErrorCode::IllegalValue => write!(f, "Illegal value"),
            ErrorCode::InvalidProcessFile => write!(f, "Invalid process file (not valid ELF)"),
            ErrorCode::OutOfMemory => write!(f, "Out of memory"),
        }
    }
}
