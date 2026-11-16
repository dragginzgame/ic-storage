use super::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
fn controlled_payloads_have_only_the_declared_scalar_fields() {
    assert_eq!((size_of::<V1>(), align_of::<V1>()), (8, 8));
    assert_eq!((size_of::<V2>(), align_of::<V2>()), (16, 8));
    assert_eq!(offset_of!(V1, counter), 0);
    assert_eq!(offset_of!(V2, counter), 0);
    assert_eq!(offset_of!(V2, ceiling), 8);
}

#[test]
fn conversion_and_mutation_validate_before_changing_values() {
    let old = V1 { counter: 41 };
    assert_eq!(old.upgrade(40), Err(Error::Ceiling));
    let mut next = old.upgrade(42).unwrap();
    next.add(1).unwrap();
    let saved = next;
    assert_eq!(next.add(1), Err(Error::Ceiling));
    assert_eq!(next, saved);
    let mut full = V1 { counter: u64::MAX }.upgrade(u64::MAX).unwrap();
    assert_eq!(full.add(1), Err(Error::Overflow));
    assert_eq!(full.counter(), u64::MAX);
}
