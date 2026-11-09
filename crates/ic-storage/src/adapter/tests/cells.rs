//! Cell envelope admission and trap-style stored codec failures.
use super::*;
use crate::StableCellPayloadError;
use std::{
    borrow::Cow,
    panic::{AssertUnwindSafe, catch_unwind},
};
#[test]
fn cell_current_header_ceiling_and_reopen_are_read_only() {
    let backing = crate::testing::MeteredMemory::<VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let key = crate::StableKey::parse("test.cell.v1").unwrap();
    let mut cell = StableCell::create(runtime.open_memory(&key).unwrap(), vec![7; 256]).unwrap();
    backing.reset();
    assert_eq!(cell.set(vec![8; 2048]), vec![7; 256]);
    let counts = backing.counts();
    assert!(!counts.overflowed);
    assert_eq!(
        (counts.writes, counts.write_bytes, counts.grows),
        (2, 2056, 0)
    );
    drop(cell);
    let before = backing.backing().borrow().clone();
    assert!(matches!(
        StableCell::<Vec<u8>, _>::load(runtime.open_memory(&key).unwrap(), 2047),
        Err(CollectionError::Cell(StableCellPayloadError::TooLarge {
            value_len: 2048
        }))
    ));
    assert_eq!(*backing.backing().borrow(), before);
    let cell = StableCell::<Vec<u8>, _>::load(runtime.open_memory(&key).unwrap(), 2048).unwrap();
    assert_eq!(cell.get(), &vec![8; 2048]);
    drop(cell);
    let memory = runtime.open_memory(&key).unwrap();
    let mut header = [0; 8];
    memory.read(0, &mut header);
    assert_eq!(&header[..4], b"SCL\x01");
    assert_eq!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 2048);
}
#[test]
fn foreign_version_and_out_of_extent_cell_lengths_never_initialize() {
    for mode in 0..3 {
        let backing = VectorMemory::default();
        let runtime = runtime(backing.clone());
        let key = crate::StableKey::parse("test.cell.v1").unwrap();
        let memory = runtime.open_memory(&key).unwrap();
        memory.grow(1).unwrap();
        let mut header = [0; 8];
        header[..4].copy_from_slice(b"SCL\x01");
        match mode {
            0 => header[0] = 0,
            1 => header[3] = 255,
            _ => header[4..8].copy_from_slice(&u32::MAX.to_le_bytes()),
        }
        memory.write(0, &header);
        drop(memory);
        let before = backing.borrow().clone();
        assert!(
            StableCell::<Vec<u8>>::load(runtime.open_memory(&key).unwrap(), usize::MAX).is_err()
        );
        assert_eq!(*backing.borrow(), before);
        assert!(runtime.open_memory(&key).is_ok());
    }
}
struct Invalid;
impl Storable for Invalid {
    const BOUND: Bound = Bound::Unbounded;
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&[1]))
    }
    fn from_bytes(_: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        Err(CodecError::Invalid("injected decode failure"))
    }
}
#[test]
fn cell_stored_decode_failure_traps_without_writing_or_leaking_the_lease() {
    let backing = VectorMemory::default();
    let runtime = runtime(backing.clone());
    let key = crate::StableKey::parse("test.cell.v1").unwrap();
    drop(StableCell::create(runtime.open_memory(&key).unwrap(), Invalid).unwrap());
    let before = backing.borrow().clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| StableCell::<Invalid>::load(
            runtime.open_memory(&key).unwrap(),
            8
        )))
        .is_err()
    );
    assert_eq!(*backing.borrow(), before);
    assert!(runtime.open_memory(&key).is_ok());
}

#[test]
fn same_length_cell_updates_preserve_framing_after_resize_and_reopen() {
    let backing = crate::testing::MeteredMemory::<VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let key = crate::StableKey::parse("test.cell.v1").unwrap();
    let mut cell = StableCell::create(runtime.open_memory(&key).unwrap(), vec![1; 32]).unwrap();
    for length in [32, 65_528, 65_529, 0, 7] {
        let previous = cell.get().clone();
        assert_eq!(cell.set(vec![2; length]), previous);
        drop(cell);
        cell = StableCell::<Vec<u8>, _>::load(runtime.open_memory(&key).unwrap(), 65_529).unwrap();
        assert_eq!(cell.get(), &vec![2; length]);
        backing.reset();
        assert_eq!(cell.set(vec![3; length]), vec![2; length]);
        let counts = backing.counts();
        assert!(!counts.overflowed);
        assert_eq!(counts.writes, u64::from(length != 0));
        assert_eq!(counts.write_bytes, length as u64);
        assert_eq!(counts.grows, 0);
        drop(cell);
        let memory = runtime.open_memory(&key).unwrap();
        assert_eq!(
            crate::stable_cell_payload_length(&memory, 65_529).unwrap(),
            length
        );
        cell = StableCell::<Vec<u8>, _>::load(memory, 65_529).unwrap();
        assert_eq!(cell.get(), &vec![3; length]);
    }
}

#[test]
fn refused_cell_growth_does_not_replace_the_cached_value() {
    let backing = ControlledMemory {
        backing: VectorMemory::default(),
        refuse_growth: Rc::new(Cell::new(false)),
        growth_calls: Rc::new(Cell::new(0)),
    };
    let runtime = runtime(backing.clone());
    let key = crate::StableKey::parse("test.cell.v1").unwrap();
    let mut cell = StableCell::create(runtime.open_memory(&key).unwrap(), vec![1; 32]).unwrap();
    backing.refuse_growth.set(true);
    assert_eq!(cell.set(vec![2; 32]), vec![1; 32]);
    let before = backing.backing.borrow().clone();
    let growth_calls = backing.growth_calls.get();
    assert!(catch_unwind(AssertUnwindSafe(|| cell.set(vec![3; 2 * 1024 * 1024]))).is_err());
    assert!(backing.growth_calls.get() > growth_calls);
    assert_eq!(cell.get(), &vec![2; 32]);
    assert_eq!(*backing.backing.borrow(), before);
}
