//! Shared non-cryptographic FNV-1a hashing for checksums and diagnostics.

pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

pub fn fnv64(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests;
