use core::fmt::Debug;
pub use core::time::*;

pub static mut GET_TIME: fn() -> Instant = || UNIX_EPOCH;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default)]
pub struct Instant {
    since_epoch: Duration,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default)]
pub struct FfiDuration {
    pub seconds: u64,
    pub nanos: u32,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default)]
pub struct FfiInstant {
    pub since_epoch: FfiDuration,
}

pub const UNIX_EPOCH: Instant = Instant {
    since_epoch: Duration::from_secs(0),
};

impl Instant {
    pub fn now() -> Instant {
        unsafe { GET_TIME() }
    }

    pub fn from_duration_since_epoch(duration: Duration) -> Instant {
        Instant { since_epoch: duration }
    }

    pub fn elapsed(&self) -> Duration {
        unsafe { GET_TIME() }.duration_since(*self)
    }

    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.since_epoch - earlier.since_epoch
    }
}

impl core::ops::Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Instant {
        Instant {
            since_epoch: self.since_epoch + rhs,
        }
    }
}

impl core::ops::Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Instant {
        Instant {
            since_epoch: self.since_epoch - rhs,
        }
    }
}

impl core::ops::Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, rhs: Instant) -> Duration {
        self.since_epoch - rhs.since_epoch
    }
}

impl Debug for Instant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Instant({:?})", self.since_epoch)
    }
}

impl core::fmt::Display for Instant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Instant({:?})", self.since_epoch)
    }
}

impl From<Instant> for FfiInstant {
    fn from(instant: Instant) -> Self {
        FfiInstant {
            since_epoch: FfiDuration {
                seconds: instant.since_epoch.as_secs(),
                nanos: instant.since_epoch.subsec_nanos(),
            },
        }
    }
}

impl From<FfiInstant> for Instant {
    fn from(ffi_instant: FfiInstant) -> Self {
        Instant {
            since_epoch: Duration::new(ffi_instant.since_epoch.seconds, ffi_instant.since_epoch.nanos),
        }
    }
}

impl From<Duration> for FfiDuration {
    fn from(duration: Duration) -> Self {
        FfiDuration {
            seconds: duration.as_secs(),
            nanos: duration.subsec_nanos(),
        }
    }
}

impl From<FfiDuration> for Duration {
    fn from(ffi_duration: FfiDuration) -> Self {
        Duration::new(ffi_duration.seconds, ffi_duration.nanos)
    }
}
