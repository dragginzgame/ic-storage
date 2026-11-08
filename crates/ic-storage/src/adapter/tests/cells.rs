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
