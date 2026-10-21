//! A small, fast, non-cryptographic hasher (the "Fx" hash used by rustc) for internal lookup
//! tables keyed by short unit/operator symbols. These maps are never keyed by attacker-chosen
//! data in a way that matters for `HashDoS` (keys are fixed registry symbols), so `SipHash`'s
//! collision resistance buys nothing while its per-lookup cost shows up in every evaluation.

use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add_to_hash(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(chunk);
            self.add_to_hash(u64::from_le_bytes(buf));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add_to_hash(u64::from_le_bytes(buf));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub(crate) type FxBuildHasher = BuildHasherDefault<FxHasher>;
pub(crate) type FxHashMap<K, V> = std::collections::HashMap<K, V, FxBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_short_keys_and_lengths() {
        let mut map: FxHashMap<&str, usize> = FxHashMap::default();
        for (i, k) in ["", "m", "mm", "m\0", "km", "kilometre", "kilometres"]
            .iter()
            .enumerate()
        {
            map.insert(k, i);
        }
        assert_eq!(map.len(), 7);
        assert_eq!(map["m"], 1);
        assert_eq!(map["m\0"], 3);
        assert_eq!(map["kilometres"], 6);
    }
}
