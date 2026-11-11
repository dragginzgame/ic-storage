//! Public Vector acceptance: models, codec calls and backing failures, without
//! private engine imports or historical format constants.
use ic_storage::{
    AllocationGrants, BackingIdentity, DirectoryError, GenericAllocationPolicy, Memory,
    MemoryAuthority, MemoryBacking, MemoryRequest, MemoryRuntime, RuntimeOpenError,
    SealedDeclarationSnapshot, StableKey, VectorMemory,
    adapter::StableVec,
    storable::{Bound, CodecError, Storable},
};
use std::{
    borrow::Cow,
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn host<M: MemoryBacking>(backing: M) -> MemoryRuntime<M> {
    let declarations = SealedDeclarationSnapshot::new(
        ["vector.a.v1", "vector.b.v1"]
            .into_iter()
            .map(|key| MemoryRequest::new("vector", key).unwrap())
            .collect(),
    )
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("vector", "vector.").unwrap()]).unwrap();
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    runtime
}
fn key() -> StableKey {
    StableKey::parse("vector.a.v1").unwrap()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Io {
    reads: usize,
    read_bytes: usize,
    writes: usize,
    grows: usize,
}
#[derive(Clone, Default)]
struct Observed {
    bytes: VectorMemory,
    io: Rc<Cell<Io>>,
    fail_write: Rc<Cell<Option<usize>>>,
    fail_read: Rc<Cell<Option<usize>>>,
    refuse_growth: Rc<Cell<bool>>,
}
impl MemoryBacking for Observed {
    fn backing_identity(&self) -> BackingIdentity {
        self.bytes.backing_identity()
    }
}
impl Memory for Observed {
    fn size(&self) -> u64 {
        self.bytes.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        let mut io = self.io.get();
        io.grows += 1;
        self.io.set(io);
        if self.refuse_growth.get() {
            -1
        } else {
            self.bytes.grow(pages)
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        let mut io = self.io.get();
        io.reads += 1;
        io.read_bytes += bytes.len();
        self.io.set(io);
        if self.fail_read.get() == Some(io.reads) {
            let partial = bytes.len().div_ceil(2);
            self.bytes.read(offset, &mut bytes[..partial]);
            panic!("injected read failure");
        }
        self.bytes.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        let mut io = self.io.get();
        io.writes += 1;
        self.io.set(io);
        if self.fail_write.get() == Some(io.writes) {
            self.bytes.write(offset, &bytes[..bytes.len().div_ceil(2)]);
            panic!("injected write failure");
        }
        self.bytes.write(offset, bytes);
    }
}
thread_local! { static DECODES: Cell<usize> = const { Cell::new(0) }; }
#[derive(Debug, PartialEq)]
struct Record<const MAX: u32, const FIXED: bool>(Vec<u8>);
impl<const MAX: u32, const FIXED: bool> Storable for Record<MAX, FIXED> {
    const BOUND: Bound = Bound::Bounded {
        max_size: MAX,
        is_fixed_size: FIXED,
    };
    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        Ok(Cow::Borrowed(&self.0))
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        DECODES.set(DECODES.get() + 1);
        Ok(Self(bytes.into_owned()))
    }
    // An application override cannot exempt bytes from the collection's bound.
    fn check_bounds(_: &[u8]) -> Result<(), CodecError> {
        Ok(())
    }
}

#[test]
fn mixed_operations_match_vec_through_cold_reopen() {
    let backing = VectorMemory::default();
    let mut expected = Vec::new();
    for generation in 0..5_u64 {
        let runtime = host(backing.clone());
        let vector = if generation == 0 {
            StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap()
        } else {
            StableVec::<u64, _>::load(runtime.open_memory(&key()).unwrap()).unwrap()
        };
        for step in 0..131_u64 {
            let value = generation * 1000 + step;
            vector.push(&value);
            expected.push(value);
            if step % 3 == 0 {
                assert_eq!(vector.pop(), expected.pop());
            }
            if !expected.is_empty() {
                vector.set(0, &value);
                expected[0] = value;
            }
            assert_eq!(vector.len(), expected.len() as u64);
        }
        assert_eq!(vector.iter().collect::<Vec<_>>(), expected);
        assert_eq!(
            vector.iter().rev().collect::<Vec<_>>(),
            expected.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(vector.get(u64::MAX), None);
        for (index, value) in expected.iter().enumerate() {
            assert_eq!(vector.get(index as u64), Some(*value));
        }
    }
}

fn widths<const MAX: u32, const FIXED: bool>() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let vector =
        StableVec::<Record<MAX, FIXED>, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    let sizes = if FIXED {
        [MAX as usize; 3]
    } else {
        [0, MAX.min(17) as usize, MAX as usize]
    };
    for (index, size) in sizes.into_iter().enumerate() {
        vector.push(&Record(vec![u8::try_from(index).unwrap(); size]));
    }
    assert_eq!(vector.pop().unwrap().0, vec![2; sizes[2]]);
    vector.push(&Record(vec![9; sizes[2]]));
    drop(vector);
    drop(runtime);
    let runtime = host(backing);
    let vector =
        StableVec::<Record<MAX, FIXED>, _>::load(runtime.open_memory(&key()).unwrap()).unwrap();
    assert_eq!(vector.len(), 3);
    assert_eq!(vector.get(2).unwrap().0, vec![9; sizes[2]]);
    assert_eq!(vector.get(0).unwrap().0, vec![0; sizes[0]]);
}

#[test]
fn finite_record_widths_include_zero_and_page_crossings() {
    widths::<0, true>();
    widths::<0, false>();
    widths::<31, true>();
    widths::<33, true>();
    widths::<255, false>();
    widths::<256, false>();
    widths::<65_535, false>();
    widths::<65_536, false>();
    widths::<65_537, true>();
}

#[test]
fn populated_reads_decode_only_the_selected_record() {
    for population in [1_u64, 512, 2048] {
        let backing = Observed::default();
        let runtime = host(backing.clone());
        let vector =
            StableVec::<Record<4096, false>, _>::create(runtime.open_memory(&key()).unwrap())
                .unwrap();
        for index in 0..population {
            let mut bytes = vec![11; 256];
            bytes[..8].copy_from_slice(&index.to_le_bytes());
            vector.push(&Record(bytes));
        }
        drop(vector);
        drop(runtime);
        let runtime = host(backing.clone());
        let vector =
            StableVec::<Record<4096, false>, _>::load(runtime.open_memory(&key()).unwrap())
                .unwrap();
        for index in [0, population / 2, population - 1] {
            backing.io.set(Io::default());
            DECODES.set(0);
            assert_eq!(&vector.get(index).unwrap().0[..8], &index.to_le_bytes());
            assert_eq!(DECODES.get(), 1);
            let io = backing.io.get();
            assert!(io.read_bytes <= 512, "population-scaled read: {io:?}");
            assert_eq!((io.writes, io.grows), (0, 0));
        }
        DECODES.set(0);
        assert!(vector.get(population).is_none());
        assert_eq!(DECODES.get(), 0);
        DECODES.set(0);
        assert_eq!(vector.iter().count(), usize::try_from(population).unwrap());
        assert_eq!(DECODES.get(), 0);
        assert_eq!(
            &vector.iter().last().unwrap().0[..8],
            &(population - 1).to_le_bytes()
        );
        assert_eq!(DECODES.get(), 1);
    }
}

#[test]
fn iterator_skips_decode_only_selected_values() {
    let runtime = host(VectorMemory::default());
    let vector =
        StableVec::<Record<8, true>, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for value in 0..100_u64 {
        vector.push(&Record(value.to_le_bytes().to_vec()));
    }
    DECODES.set(0);
    let mut actual = vector.iter();
    let values: Vec<_> = (0..100_u64).collect();
    let mut expected = values.into_iter();
    assert_eq!(
        actual.nth(17).unwrap().0,
        expected.nth(17).unwrap().to_le_bytes()
    );
    assert_eq!(
        actual.nth_back(13).unwrap().0,
        expected.nth_back(13).unwrap().to_le_bytes()
    );
    assert_eq!(DECODES.get(), 2);
    assert_eq!(actual.size_hint(), expected.size_hint());
    assert_eq!(actual.count(), expected.count());
    assert_eq!(DECODES.get(), 2);
    let mut exhausted = vector.iter();
    assert!(exhausted.nth(usize::MAX).is_none());
    assert!(exhausted.next().is_none());
    assert!(exhausted.next_back().is_none());
    assert_eq!(DECODES.get(), 2);
}

#[test]
fn failed_admission_is_read_only_and_releases_lease() {
    let backing = Observed::default();
    let runtime = host(backing.clone());
    let before = backing.bytes.borrow().clone();
    assert!(StableVec::<u64, _>::load(runtime.open_memory(&key()).unwrap()).is_err());
    assert_eq!(*backing.bytes.borrow(), before);
    let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    vector.push(&19);
    drop(vector);
    let before = backing.bytes.borrow().clone();
    backing.io.set(Io::default());
    assert!(StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(|| StableVec::<u32, _>::load(
            runtime.open_memory(&key()).unwrap()
        )))
        .is_err()
    );
    assert_eq!(*backing.bytes.borrow(), before);
    assert_eq!((backing.io.get().writes, backing.io.get().grows), (0, 0));
    assert_eq!(
        StableVec::<u64, _>::load(runtime.open_memory(&key()).unwrap())
            .unwrap()
            .get(0),
        Some(19)
    );
}

#[test]
fn bounds_are_enforced_even_when_codec_helpers_claim_success() {
    for push in [false, true] {
        let backing = Observed::default();
        let runtime = host(backing.clone());
        let vector =
            StableVec::<Record<8, true>, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
        vector.push(&Record(vec![1; 8]));
        backing.io.set(Io::default());
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if push {
                    vector.push(&Record(vec![2; 7]));
                } else {
                    vector.set(0, &Record(vec![2; 9]));
                }
            }))
            .is_err()
        );
        assert_eq!(backing.io.get().writes, 0);
        assert!(catch_unwind(AssertUnwindSafe(|| vector.len())).is_err());
    }
}

#[test]
fn explicit_reset_preserves_the_neighbor() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let other = StableKey::parse("vector.b.v1").unwrap();
    let a = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    let b = StableVec::<u64, _>::create(runtime.open_memory(&other).unwrap()).unwrap();
    for index in 0..8193 {
        a.push(&index);
        b.push(&(index + 17));
    }
    drop(a);
    drop(b);
    assert!(
        StableVec::<u64, _>::reset(runtime.open_memory(&key()).unwrap())
            .unwrap()
            .is_empty()
    );
    drop(runtime);
    let runtime = host(backing);
    assert_eq!(
        StableVec::<u64, _>::load(runtime.open_memory(&other).unwrap())
            .unwrap()
            .get(8192),
        Some(8209)
    );
}

#[test]
fn unbounded_types_are_refused() {
    let runtime = host(VectorMemory::default());
    assert!(
        catch_unwind(AssertUnwindSafe(|| StableVec::<Vec<u8>, _>::create(
            runtime.open_memory(&key()).unwrap()
        )))
        .is_err()
    );
}

#[test]
fn every_push_write_failure_poisoning_precedes_further_access() {
    let prepare = || {
        let backing = Observed::default();
        let runtime = host(backing.clone());
        let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
        vector.push(&17);
        backing.io.set(Io::default());
        (backing, runtime, vector)
    };
    let (probe, _runtime, vector) = prepare();
    vector.push(&29);
    let writes = probe.io.get().writes;
    assert!(writes > 0);
    for failure in 1..=writes {
        let (backing, runtime, vector) = prepare();
        backing.fail_write.set(Some(failure));
        assert!(catch_unwind(AssertUnwindSafe(|| vector.push(&29))).is_err());
        let io = backing.io.get();
        assert_eq!(io.writes, failure);
        assert!(catch_unwind(AssertUnwindSafe(|| vector.len())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| vector.get(0))).is_err());
        assert!(matches!(
            runtime.open_memory(&key()),
            Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
        ));
        assert_eq!(backing.io.get(), io);
    }
}

#[test]
fn iteration_observes_replacements_and_clamps_after_removal() {
    let runtime = host(VectorMemory::default());
    let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
    for value in 0..5 {
        vector.push(&value);
    }
    let mut iter = vector.iter();
    assert_eq!(iter.next(), Some(0));
    vector.set(1, &19);
    vector.push(&5);
    assert_eq!(iter.next(), Some(19));
    assert_eq!(vector.pop(), Some(5));
    assert_eq!(vector.pop(), Some(4));
    assert_eq!(iter.next_back(), Some(3));
    vector.push(&99);
    assert_eq!(iter.collect::<Vec<_>>(), vec![2]);
}

#[test]
fn poisoned_owner_is_observed_on_selected_missing_and_exhausted_paths() {
    for populated in [false, true] {
        let backing = Observed::default();
        let runtime = host(backing.clone());
        let vector = StableVec::<u64, _>::create(runtime.open_memory(&key()).unwrap()).unwrap();
        if populated {
            vector.push(&17);
        }
        let mut forward = vector.iter();
        let mut reverse = vector.iter();
        let mut skipped = vector.iter();
        let mut skipped_back = vector.iter();
        let counted = vector.iter();
        let hinted = vector.iter();
        let other = runtime
            .open_memory(&StableKey::parse("vector.b.v1").unwrap())
            .unwrap();
        assert_eq!(other.grow(1).unwrap(), 0);
        backing.io.set(Io::default());
        backing.fail_write.set(Some(1));
        assert!(catch_unwind(AssertUnwindSafe(|| other.write(0, &[1]))).is_err());
        let io = backing.io.get();
        assert!(catch_unwind(AssertUnwindSafe(|| vector.get(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| vector.get(u64::MAX))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| vector.pop())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| forward.next())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| reverse.next_back())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| skipped.nth(usize::MAX))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| skipped_back.nth_back(usize::MAX))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| counted.count())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| hinted.size_hint())).is_err());
        assert_eq!(backing.io.get(), io, "poison refusal precedes backing IO");
    }
}

#[test]
fn interrupted_reads_and_refused_growth_invalidate_owner() {
    for read_failure in [true, false] {
        let backing = Observed::default();
        let runtime = host(backing.clone());
        let vector =
            StableVec::<Record<1_048_576, false>, _>::create(runtime.open_memory(&key()).unwrap())
                .unwrap();
        vector.push(&Record(vec![7; 256]));
        backing.io.set(Io::default());
        if read_failure {
            backing.fail_read.set(Some(1));
        } else {
            backing.refuse_growth.set(true);
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if read_failure {
                    let _ = vector.get(0);
                } else {
                    vector.push(&Record(vec![9; 1_048_576]));
                }
            }))
            .is_err()
        );
        let io = backing.io.get();
        assert!(catch_unwind(AssertUnwindSafe(|| vector.is_empty())).is_err());
        assert!(matches!(
            runtime.open_memory(&key()),
            Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
        ));
        assert_eq!(backing.io.get(), io);
    }
}
