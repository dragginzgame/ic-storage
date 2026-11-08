//! Population-scaled IO and the scalar ICSS iterator contract.
use super::*;
use crate::{
    StableKey, StorageConfig,
    testing::{MeteredMemory, fixture_runtime},
};
use std::{
    borrow::Cow,
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
thread_local! {
    static DECODES:Cell<(u64,usize)>=const{Cell::new((0,0))};
    static BORROWED_DECODES: Cell<u64> = const { Cell::new(0) };
}
struct Record<const MAXIMUM: u32 = 4096, const FIXED: bool = false>(Vec<u8>);
impl<const MAXIMUM: u32, const FIXED: bool> Storable for Record<MAXIMUM, FIXED> {
    const BOUND: super::super::Bound = super::super::Bound::Bounded {
        max_size: MAXIMUM,
        is_fixed_size: FIXED,
    };
    fn to_bytes(&self) -> std::result::Result<Cow<'_, [u8]>, super::super::CodecError> {
        Ok(Cow::Borrowed(&self.0))
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> std::result::Result<Self, super::super::CodecError> {
        if matches!(&bytes, Cow::Borrowed(_)) {
            BORROWED_DECODES.set(BORROWED_DECODES.get() + 1);
        }
        DECODES.with(|x| {
            let (c, n) = x.get();
            x.set((c + 1, n + bytes.len()));
        });
        Ok(Self(bytes.into_owned()))
    }
}
fn runtime<M: crate::MemoryBacking>(backing: M) -> crate::MemoryRuntime<M> {
    fixture_runtime(
        backing,
        "test",
        "test.",
        &["test.vector.v1"],
        StorageConfig::default(),
    )
    .unwrap()
}
fn key() -> StableKey {
    StableKey::parse("test.vector.v1").unwrap()
}

#[test]
fn header_open_reads_once_without_writing_or_caching_length() {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    vector.push(&73);
    drop(vector);
    let memory = runtime.open_memory(&key()).unwrap();
    backing.reset();
    let vector = StableVec::<u64, _>::load(memory).unwrap();
    let io = backing.counts();
    assert_eq!(
        (io.reads, io.read_bytes, io.writes, io.grows),
        (1, 17, 0, 0)
    );
    assert_eq!(vector.get(0), Some(73));
    backing.reset();
    assert_eq!(vector.len(), 1);
    assert_eq!(
        (backing.counts().reads, backing.counts().read_bytes),
        (1, 8)
    );
}

#[test]
fn header_reset_preserves_reserved_bytes_and_payload() {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let memory = runtime.open_memory(&key()).unwrap();
    assert_eq!(memory.grow(1).unwrap(), 0);
    memory.write(17, &[0xa5; 55]);
    backing.reset();
    let vector = StableVec::<u64, _>::reset(memory).unwrap();
    let io = backing.counts();
    assert_eq!(
        (io.reads, io.writes, io.write_bytes, io.grows),
        (0, 1, 17, 0)
    );
    drop(vector);
    let memory = runtime.open_memory(&key()).unwrap();
    let mut bytes = [0; 72];
    memory.read(0, &mut bytes);
    assert_eq!(&bytes[..4], b"SVC\x01");
    assert_eq!(&bytes[4..12], &0_u64.to_le_bytes());
    assert_eq!(&bytes[12..16], &8_u32.to_le_bytes());
    assert_eq!(bytes[16], 1);
    assert_eq!(&bytes[17..], &[0xa5; 55]);
}

#[test]
fn header_admission_preserves_typed_errors_traps_bytes_and_lease_release() {
    for (offset, byte, traps) in [
        (0, b'X', false),
        (3, 2, false),
        (12, 9, true),
        (16, 0, true),
    ] {
        let backing = MeteredMemory::<crate::VectorMemory>::default();
        let runtime = runtime(backing.clone());
        drop(StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap());
        let memory = runtime.open_memory(&key()).unwrap();
        memory.write(offset, &[byte]);
        let before = backing.backing().borrow().clone();
        backing.reset();
        let result = catch_unwind(AssertUnwindSafe(|| StableVec::<u64, _>::load(memory)));
        if traps {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(Error::Format))));
        }
        assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
        assert_eq!(*backing.backing().borrow(), before);
        assert!(runtime.open_memory(&key()).is_ok());
    }
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let memory = runtime.open_memory(&key()).unwrap();
    backing.reset();
    assert!(matches!(
        StableVec::<u64, _>::load(memory),
        Err(Error::Format)
    ));
    assert_eq!(backing.counts(), crate::testing::MemoryIoCounts::default());
    drop(StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap());
    let memory = runtime.open_memory(&key()).unwrap();
    // ICSS interprets any nonzero fixed-size flag as true.
    memory.write(16, &[2]);
    assert!(StableVec::<u64, _>::load(memory).unwrap().is_empty());
}

#[test]
fn populated_lookups_read_one_length_one_slot_and_decode_once_after_reopen() {
    for payload in [256, 2048] {
        for population in [1u64, 512, 1024, 2048] {
            let backing = MeteredMemory::<crate::VectorMemory>::default();
            let runtime = runtime(backing.clone());
            let vector =
                StableVec::<Record, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
            for id in 0..population {
                let mut bytes = vec![42; payload];
                bytes[..8].copy_from_slice(&id.to_le_bytes());
                vector.push(&Record(bytes));
            }
            drop(vector);
            let vector =
                StableVec::<Record, _>::load(runtime.open_memory(&key()).unwrap()).unwrap();
            for index in [0, population / 2, population - 1] {
                backing.reset();
                DECODES.with(|x| x.set((0, 0)));
                let item = vector.get(index).unwrap();
                assert_eq!(&item.0[..8], &index.to_le_bytes());
                let io = backing.counts();
                assert!(!io.overflowed);
                assert!((3..=5).contains(&io.reads));
                if index == 0 {
                    assert_eq!(io.reads, 3);
                }
                assert_eq!(io.read_bytes, payload as u64 + 2 + 8);
                assert_eq!((io.writes, io.grows), (0, 0));
                DECODES.with(|x| assert_eq!(x.get(), (1, payload)));
            }
            backing.reset();
            DECODES.with(|x| x.set((0, 0)));
            assert!(vector.get(population).is_none());
            assert_eq!(
                (backing.counts().reads, backing.counts().read_bytes),
                (1, 8)
            );
            DECODES.with(|x| assert_eq!(x.get(), (0, 0)));
        }
    }
}
fn check_geometry<const MAXIMUM: u32, const FIXED: bool>(prefix: u64) {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let vector =
        StableVec::<Record<MAXIMUM, FIXED>, _>::create(runtime.open_memory(&key()).unwrap())
            .unwrap();
    let sizes = if FIXED {
        vec![MAXIMUM as usize; 3]
    } else {
        vec![0, MAXIMUM.min(32) as usize, MAXIMUM as usize]
    };
    for (i, &n) in sizes.iter().enumerate() {
        vector.push(&Record(vec![u8::try_from(i).unwrap(); n]));
    }
    for (i, &n) in sizes.iter().enumerate() {
        backing.reset();
        assert_eq!(
            vector.get(i as u64).unwrap().0,
            vec![u8::try_from(i).unwrap(); n]
        );
        assert_eq!(backing.counts().read_bytes, n as u64 + prefix + 8);
    }
    assert_eq!(vector.pop().unwrap().0, vec![2; sizes[2]]);
    drop(vector);
    let vector =
        StableVec::<Record<MAXIMUM, FIXED>, _>::load(runtime.open_memory(&key()).unwrap()).unwrap();
    assert_eq!(vector.len(), 2);
}
#[test]
fn scalar_geometry_covers_length_widths_and_zero_sized_fixed_records() {
    check_geometry::<0, false>(1);
    check_geometry::<255, false>(1);
    check_geometry::<256, false>(2);
    check_geometry::<65535, false>(2);
    check_geometry::<65536, false>(4);
    check_geometry::<0, true>(0);
    check_geometry::<32, true>(0);
}
fn check_fixed_array<const WIDTH: usize>() {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let vector = StableVec::<[u8; WIDTH], _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for byte in [3, 7, 11] {
        vector.push(&[byte; WIDTH]);
    }
    backing.reset();
    assert_eq!(vector.get(1), Some([7; WIDTH]));
    // Read exactly the encoded width, including zero and either side of 32.
    assert_eq!(backing.counts().read_bytes, 8 + WIDTH as u64);
    vector.set(1, &[19; WIDTH]);
    assert_eq!(vector.pop(), Some([11; WIDTH]));
    drop(vector);
    let vector = StableVec::<[u8; WIDTH], _>::load(runtime.open_memory(&key()).unwrap()).unwrap();
    assert_eq!(vector.len(), 2);
    assert_eq!(vector.get(1), Some([19; WIDTH]));
    assert_eq!(vector.get(2), None);
}
#[test]
fn fixed_arrays_read_exact_width_across_scratch_boundary_and_reopen() {
    check_fixed_array::<0>();
    check_fixed_array::<1>();
    check_fixed_array::<8>();
    check_fixed_array::<31>();
    check_fixed_array::<32>();
    check_fixed_array::<33>();
}
#[test]
fn fixed_heap_owned_records_keep_the_owned_decode_handoff() {
    let runtime = runtime(crate::VectorMemory::default());
    let vector =
        StableVec::<Record<32, true>, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    vector.push(&Record(vec![5; 32]));
    BORROWED_DECODES.set(0);
    let saved = vector.get(0).unwrap();
    vector.set(0, &Record(vec![9; 32]));
    assert_eq!(saved.0, vec![5; 32]);
    assert_eq!(vector.pop().unwrap().0, vec![9; 32]);
    // The decoder can take the read buffer instead of allocating and copying it.
    assert_eq!(BORROWED_DECODES.get(), 0);
}
#[test]
fn iterators_see_writes_and_match_upstream_truncation_and_skipping() {
    let runtime = runtime(crate::VectorMemory::default());
    let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for value in 0..20 {
        vector.push(&value);
    }
    let mut iter = vector.iter();
    assert_eq!(iter.next().unwrap(), 0);
    vector.set(1, &99);
    assert_eq!(iter.next().unwrap(), 99);
    assert_eq!(vector.pop(), Some(19));
    assert!(iter.next_back().is_none());
    vector.push(&77);
    assert_eq!(iter.next_back().unwrap(), 77);
    assert_eq!(iter.nth(2).unwrap(), 4);
    assert_eq!(iter.nth_back(2).unwrap(), 16);
    assert_eq!(iter.last().unwrap(), 15);
    assert_eq!(
        vector.iter().rev().collect::<Vec<_>>(),
        [vec![77], (2..19).rev().collect(), vec![99, 0]].concat()
    );
}
#[test]
fn iterator_skips_and_count_delegate_without_decoding_discarded_records() {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let vector = StableVec::<Record, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for id in 0u64..256 {
        let mut bytes = vec![42; 256];
        bytes[..8].copy_from_slice(&id.to_le_bytes());
        vector.push(&Record(bytes));
    }
    for reverse in [false, true] {
        backing.reset();
        DECODES.with(|x| x.set((0, 0)));
        let mut iter = vector.iter();
        let item = if reverse {
            iter.nth_back(254)
        } else {
            iter.nth(254)
        };
        let expected = if reverse { 1u64 } else { 254u64 };
        assert_eq!(&item.unwrap().0[..8], &expected.to_le_bytes());
        DECODES.with(|x| assert_eq!(x.get(), (1, 256)));
        assert_eq!(backing.counts().read_bytes, 8 + 8 + 2 + 256);
        assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
    }
    backing.reset();
    DECODES.with(|x| x.set((0, 0)));
    assert_eq!(vector.iter().count(), 256);
    DECODES.with(|x| assert_eq!(x.get(), (0, 0)));
    assert_eq!(backing.counts().read_bytes, 16);
}
#[test]
fn malformed_slot_length_traps_before_payload_allocation_or_decode() {
    let backing = MeteredMemory::<crate::VectorMemory>::default();
    let runtime = runtime(backing.clone());
    let vector = StableVec::<Record, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    vector.push(&Record(vec![7; 256]));
    vector.inner.memory().write(64, &4097u16.to_le_bytes());
    backing.reset();
    DECODES.with(|x| x.set((0, 0)));
    assert!(catch_unwind(AssertUnwindSafe(|| vector.get(0))).is_err());
    assert_eq!(backing.counts().read_bytes, 10);
    assert_eq!(backing.counts().writes, 0);
    DECODES.with(|x| assert_eq!(x.get(), (0, 0)));
}

#[test]
fn last_matches_forward_iteration_after_consumption_and_length_changes() {
    for initial in [0u64, 1, 8] {
        for front in [0, 1, 3, 9] {
            for back in [0, 1, 3, 9] {
                for final_len in [0, 1, 4, 8, 12] {
                    let runtime = runtime(crate::VectorMemory::default());
                    let vector =
                        StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
                    for value in 0..initial {
                        vector.push(&value);
                    }
                    let mut direct = vector.iter();
                    let mut forward = vector.iter();
                    for _ in 0..front {
                        assert_eq!(direct.next(), forward.next());
                    }
                    for _ in 0..back {
                        assert_eq!(direct.next_back(), forward.next_back());
                    }
                    while vector.len() > final_len {
                        let _ = vector.pop();
                    }
                    while vector.len() < final_len {
                        vector.push(&(100 + vector.len()));
                    }
                    let expected = forward.fold(None, |_, value| Some(value));
                    assert_eq!(direct.last(), expected);
                }
            }
        }
    }
}

#[test]
fn last_reads_and_decodes_only_the_selected_slot() {
    for population in [1u64, 256, 2048] {
        let backing = MeteredMemory::<crate::VectorMemory>::default();
        let runtime = runtime(backing.clone());
        let vector = StableVec::<Record, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
        for id in 0..population {
            vector.push(&Record(id.to_le_bytes().to_vec()));
        }
        backing.reset();
        DECODES.with(|x| x.set((0, 0)));
        assert_eq!(
            vector.iter().last().unwrap().0,
            (population - 1).to_le_bytes()
        );
        DECODES.with(|x| assert_eq!(x.get(), (1, 8)));
        assert_eq!(backing.counts().read_bytes, 8 + 8 + 2 + 8);
        assert_eq!((backing.counts().writes, backing.counts().grows), (0, 0));
    }
}
