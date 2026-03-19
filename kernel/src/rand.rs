static mut RND_STATE: Xoshiro128PlusPlus = Xoshiro128PlusPlus { state: [0; 4] };

pub fn init_rand() {
    let mut rnd_low: u64;
    let mut rnd_high: u64;
    unsafe {
        core::arch::asm!(
            "rdseed {0:e}",
            "rdseed {1:e}",
            out(reg) rnd_low,
            out(reg) rnd_high,
            options(nostack, nomem)
        );
    }
    unsafe {
        RND_STATE = Xoshiro128PlusPlus::from_seed([
            (rnd_low & 0xFFFFFFFF) as u32,
            ((rnd_low >> 32) & 0xFFFFFFFF) as u32,
            (rnd_high & 0xFFFFFFFF) as u32,
            ((rnd_high >> 32) & 0xFFFFFFFF) as u32,
        ]);
    }
}

#[derive(Clone, Copy)]
struct Xoshiro128PlusPlus {
    state: [u32; 4],
}

impl Xoshiro128PlusPlus {
    /// Seed the PRNG with 4 u32 values
    pub fn from_seed(seed: [u32; 4]) -> Self {
        assert!(seed != [0; 4], "Seed cannot be all zeros");
        Self { state: seed }
    }

    /// Rotate left
    #[inline]
    fn rotl(x: u32, k: u32) -> u32 {
        x.rotate_left(k)
    }

    /// Generate next random u32
    pub fn next_u32(&mut self) -> u32 {
        let result = Self::rotl(self.state[0].wrapping_add(self.state[3]), 7).wrapping_add(self.state[0]);

        let t = self.state[1] << 9;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];

        self.state[2] ^= t;
        self.state[3] = Self::rotl(self.state[3], 11);

        result
    }

    /// Generate next u64 by combining two u32 outputs
    pub fn next_u64(&mut self) -> u64 {
        let high = self.next_u32() as u64;
        let low = self.next_u32() as u64;
        (high << 32) | low
    }
}

pub fn rand_u32() -> u32 {
    unsafe { RND_STATE.next_u32() }
}

pub fn rand_u64() -> u64 {
    unsafe { RND_STATE.next_u64() }
}

pub fn rand_u8() -> u8 {
    (rand_u32() & 0xFF) as u8
}

pub fn rand_u16() -> u16 {
    (rand_u32() & 0xFFFF) as u16
}
