//! A tiny FNV-1a `Hasher` so position hashing (see `impls::position_hash`)
//! doesn't need `std`'s `DefaultHasher` — this crate is `no_std` + `alloc`,
//! and SipHash's implementation lives in `std`, not `core`.
use core::hash::Hasher;

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

pub struct Fnv1aHasher(u64);

impl Default for Fnv1aHasher {
    fn default() -> Self {
        Fnv1aHasher(FNV_OFFSET_BASIS)
    }
}

impl Hasher for Fnv1aHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}
