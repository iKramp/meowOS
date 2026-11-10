use core::fmt::Debug;
pub use core::time::*;

pub static mut GET_TIME: fn() -> Instant = || UNIX_EPOCH;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default)]
pub struct Instant {
    since_epoch: Duration,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default, Debug)]
pub struct UsDuration {
    pub microseconds: i64, //able to represent negative durations
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default, Debug)]
//same as rust but with repr C
pub struct FfiDuration {
    pub seconds: u64,
    pub nanoseconds: u32,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default, Debug)]
pub struct FfiInstant {
    pub since_epoch: FfiDuration,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default, Debug)]
pub struct UsInstant {
    pub since_epoch: UsDuration,
}

pub struct AtomicDuration {
    duration: core::sync::atomic::AtomicI64,
}

pub struct AtomicInstant {
    instant: core::sync::atomic::AtomicI64,
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

impl UsDuration {
    pub fn from_micros(microseconds: i64) -> Self {
        UsDuration { microseconds }
    }
}

impl UsInstant {
    pub fn from_micros(microseconds: i64) -> Self {
        UsInstant {
            since_epoch: UsDuration { microseconds },
        }
    }
}

impl core::ops::Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Instant {
        Instant {
            since_epoch: self.since_epoch.saturating_add(rhs),
        }
    }
}

impl core::ops::Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, rhs: Duration) -> Instant {
        Instant {
            since_epoch: self.since_epoch.saturating_sub(rhs),
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

impl From<Instant> for UsInstant {
    fn from(instant: Instant) -> Self {
        let duration: UsDuration = instant.since_epoch.into();
        UsInstant { since_epoch: duration }
    }
}

impl From<Instant> for FfiInstant {
    fn from(instant: Instant) -> Self {
        let duration: FfiDuration = instant.since_epoch.into();
        FfiInstant { since_epoch: duration }
    }
}

impl From<UsInstant> for Instant {
    fn from(ffi_instant: UsInstant) -> Self {
        let duration: Duration = ffi_instant.since_epoch.into();
        Instant { since_epoch: duration }
    }
}

impl From<FfiInstant> for Instant {
    fn from(ffi_instant: FfiInstant) -> Self {
        let duration: Duration = ffi_instant.since_epoch.into();
        Instant { since_epoch: duration }
    }
}

impl From<Duration> for UsDuration {
    fn from(duration: Duration) -> Self {
        let mut micros = 0;
        micros += duration.as_secs() as i64 * 1_000_000;
        micros += duration.subsec_micros() as i64;
        UsDuration { microseconds: micros }
    }
}

impl From<Duration> for FfiDuration {
    fn from(duration: Duration) -> Self {
        FfiDuration {
            seconds: duration.as_secs(),
            nanoseconds: duration.subsec_nanos(),
        }
    }
}

impl From<UsDuration> for Duration {
    fn from(ffi_duration: UsDuration) -> Self {
        let seconds = ffi_duration.microseconds / 1_000_000;
        let nanos = (ffi_duration.microseconds % 1_000_000) * 1_000;
        Duration::new(seconds as u64, nanos as u32)
    }
}

impl From<FfiDuration> for Duration {
    fn from(ffi_duration: FfiDuration) -> Self {
        Duration::new(ffi_duration.seconds, ffi_duration.nanoseconds)
    }
}

impl AtomicDuration {
    pub fn new(duration: UsDuration) -> Self {
        let micros = duration.microseconds;
        AtomicDuration {
            duration: core::sync::atomic::AtomicI64::new(micros),
        }
    }

    pub fn load(&self, ordering: core::sync::atomic::Ordering) -> UsDuration {
        let micros = self.duration.load(ordering);
        UsDuration { microseconds: micros }
    }

    pub fn store(&self, duration: UsDuration, ordering: core::sync::atomic::Ordering) {
        let micros = duration.microseconds;
        self.duration.store(micros, ordering);
    }

    pub fn fetch_add(&self, duration: UsDuration, ordering: core::sync::atomic::Ordering) -> UsDuration {
        let micros = duration.microseconds;
        let prev_micros = self.duration.fetch_add(micros, ordering);
        UsDuration {
            microseconds: prev_micros,
        }
    }
}

impl AtomicInstant {
    pub fn new(instant: UsInstant) -> Self {
        let micros = instant.since_epoch.microseconds;
        AtomicInstant {
            instant: core::sync::atomic::AtomicI64::new(micros),
        }
    }

    pub fn load(&self, ordering: core::sync::atomic::Ordering) -> UsInstant {
        let micros = self.instant.load(ordering);
        UsInstant {
            since_epoch: UsDuration { microseconds: micros },
        }
    }

    pub fn store(&self, instant: UsInstant, ordering: core::sync::atomic::Ordering) {
        let micros = instant.since_epoch.microseconds;
        self.instant.store(micros, ordering);
    }

    pub fn fetch_add(&self, duration: UsDuration, ordering: core::sync::atomic::Ordering) -> UsInstant {
        let micros = duration.microseconds;
        let prev_micros = self.instant.fetch_add(micros, ordering);
        UsInstant {
            since_epoch: UsDuration {
                microseconds: prev_micros,
            },
        }
    }
}
