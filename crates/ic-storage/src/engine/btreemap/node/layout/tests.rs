//! Existing node bytes, combined prefix reads and corruption admission.
use super::*;
use crate::{VectorMemory, testing::MeteredMemory};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn packed_prefix_preserves_unaligned_headers_and_overflow_links() {
    let memory = MeteredMemory::<VectorMemory>::default();
    memory.grow(2);
    let at = 65_530;
    let mut prefix = [0; 15];
    prefix[..7].copy_from_slice(b"BTN\x02\x00\x02\x00");
    memory.write(at, &prefix);
    memory.reset();
    let (header, overflows) = read_header_and_overflows(Address::from(at), &memory);
    let entries = header.num_entries;
    assert_eq!(entries, 2);
    assert_eq!(overflows, [] as [Address; 0]);
    assert_eq!(memory.counts().reads, 1);
    assert_eq!(memory.counts().read_bytes, 15);

    prefix[7..].copy_from_slice(&17_u64.to_le_bytes());
    memory.write(at, &prefix);
    memory.write(17, b"NOF");
    memory.write(20, &40_u64.to_le_bytes());
    memory.write(40, b"NOF");
    memory.write(43, &0_u64.to_le_bytes());
    memory.reset();
    let (_, overflows) = read_header_and_overflows(Address::from(at), &memory);
    assert_eq!(overflows, [Address::from(17), Address::from(40)]);
    assert_eq!(memory.counts().reads, 3);
    assert_eq!(memory.counts().read_bytes, 37);
}

#[test]
fn corrupt_prefix_is_rejected_before_following_its_overflow_link() {
    let memory = MeteredMemory::<VectorMemory>::default();
    memory.grow(1);
    for bytes in [b"BAD\x02\x00\x00\x00", b"BTN\x03\x00\x00\x00"] {
        memory.write(0, bytes);
        memory.write(7, &u64::MAX.to_le_bytes());
        memory.reset();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                read_header_and_overflows(Address::from(0), &memory)
            }))
            .is_err()
        );
        assert_eq!(memory.counts().reads, 1);
        assert_eq!(memory.counts().read_bytes, 15);
    }
}
