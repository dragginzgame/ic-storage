//! Public behavioral acceptance for an independently implemented Cell.
//! No engine imports, historical layout bytes, or ICSS comparison dependency.
//! Authored in a source-exposed session; not a clean-room authorship certificate.
use ic_storage::{
    AllocationGrants, BackingIdentity, DirectoryError, GenericAllocationPolicy, Memory,
    MemoryAuthority, MemoryBacking, MemoryRequest, MemoryRuntime, RuntimeOpenError,
    SealedDeclarationSnapshot, StableKey, VectorMemory,
    adapter::StableCell,
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
        ["contract.a.v1", "contract.b.v1"]
            .into_iter()
            .map(|name| MemoryRequest::new("contract", name).unwrap())
            .collect(),
    )
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("contract", "contract.").unwrap()])
            .unwrap();
    let mut runtime = MemoryRuntime::new(backing).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    runtime
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Io {
    sizes: usize,
    grows: usize,
    reads: usize,
    read_bytes: usize,
    writes: usize,
}

#[derive(Clone, Default)]
struct ObservedMemory {
    bytes: VectorMemory,
    io: Rc<Cell<Io>>,
    fail_write: Rc<Cell<Option<usize>>>,
    fail_read: Rc<Cell<Option<usize>>>,
    refuse_growth: Rc<Cell<bool>>,
}

impl MemoryBacking for ObservedMemory {
    fn backing_identity(&self) -> BackingIdentity {
        self.bytes.backing_identity()
    }
}

impl Memory for ObservedMemory {
    fn size(&self) -> u64 {
        let mut io = self.io.get();
        io.sizes += 1;
        self.io.set(io);
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
            panic!("injected partial read");
        }
        self.bytes.read(offset, bytes);
    }

    fn write(&self, offset: u64, bytes: &[u8]) {
        let mut io = self.io.get();
        io.writes += 1;
        self.io.set(io);
        if self.fail_write.get() == Some(io.writes) {
            self.bytes.write(offset, &bytes[..bytes.len().div_ceil(2)]);
            panic!("injected partial write");
        }
        self.bytes.write(offset, bytes);
    }
}

thread_local! {
    static ENCODE_CALLS: Cell<usize> = const { Cell::new(0) };
    static DECODE_CALLS: Cell<usize> = const { Cell::new(0) };
    static REFUSE_DECODE: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, PartialEq)]
struct Record {
    bytes: Vec<u8>,
    refuse_encode: bool,
}

impl Record {
    const fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            refuse_encode: false,
        }
    }
}

impl Storable for Record {
    const BOUND: Bound = Bound::Unbounded;

    fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
        ENCODE_CALLS.set(ENCODE_CALLS.get() + 1);
        if self.refuse_encode {
            Err(CodecError::Invalid("injected encoding failure"))
        } else {
            Ok(Cow::Borrowed(&self.bytes))
        }
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
        DECODE_CALLS.set(DECODE_CALLS.get() + 1);
        if REFUSE_DECODE.get() {
            Err(CodecError::Invalid("injected decoding failure"))
        } else {
            Ok(Self::new(bytes.into_owned()))
        }
    }
}

#[test]
fn cached_reads_and_drop_do_not_call_backing_or_codec() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let cell = StableCell::create(
        runtime.open_memory(&key).unwrap(),
        Record::new(vec![19; 70_001]),
    )
    .unwrap();
    let io = backing.io.get();
    let codecs = (ENCODE_CALLS.get(), DECODE_CALLS.get());
    for _ in 0..32 {
        assert_eq!(cell.get().bytes.len(), 70_001);
        assert!(cell.get().bytes.iter().all(|byte| *byte == 19));
    }
    drop(cell);
    assert_eq!(backing.io.get(), io);
    assert_eq!((ENCODE_CALLS.get(), DECODE_CALLS.get()), codecs);
    assert!(runtime.open_memory(&key).is_ok());
}

#[test]
fn empty_replacement_validates_codec_and_health_without_backing_io() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let mut cell =
        StableCell::create(runtime.open_memory(&key).unwrap(), Record::new(Vec::new())).unwrap();
    backing.io.set(Io::default());
    let encodes = ENCODE_CALLS.get();
    assert_eq!(cell.set(Record::new(Vec::new())).bytes, Vec::<u8>::new());
    assert_eq!(ENCODE_CALLS.get(), encodes + 1);
    assert_eq!(backing.io.get(), Io::default());
    assert!(
        catch_unwind(AssertUnwindSafe(|| cell.set(Record {
            bytes: Vec::new(),
            refuse_encode: true,
        })))
        .is_err()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| cell.set(Record::new(Vec::new())))).is_err());
    assert_eq!(backing.io.get(), Io::default());
}

#[test]
fn ceiling_refusal_precedes_payload_read_and_decode() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    drop(
        StableCell::create(
            runtime.open_memory(&key).unwrap(),
            Record::new(vec![17; 70_001]),
        )
        .unwrap(),
    );
    let memory = runtime.open_memory(&key).unwrap();
    let before = backing.bytes.borrow().clone();
    let decodes = DECODE_CALLS.get();
    backing.io.set(Io::default());
    assert!(StableCell::<Record, _>::load(memory, 70_000).is_err());
    let io = backing.io.get();
    assert_eq!((io.writes, io.grows), (0, 0));
    assert!(io.read_bytes < 70_001);
    assert_eq!(DECODE_CALLS.get(), decodes);
    assert_eq!(*backing.bytes.borrow(), before);
    backing.io.set(Io::default());
    let cell = StableCell::<Record, _>::load(runtime.open_memory(&key).unwrap(), 70_001).unwrap();
    assert_eq!(cell.get().bytes, vec![17; 70_001]);
    let io = backing.io.get();
    assert_eq!((io.writes, io.grows), (0, 0));
    assert_eq!(*backing.bytes.borrow(), before);
}

#[test]
fn failed_decode_preserves_backing_and_releases_lease() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    drop(StableCell::create(runtime.open_memory(&key).unwrap(), Record::new(vec![5; 37])).unwrap());
    let before = backing.bytes.borrow().clone();
    backing.io.set(Io::default());
    REFUSE_DECODE.set(true);
    let result = catch_unwind(AssertUnwindSafe(|| {
        StableCell::<Record, _>::load(runtime.open_memory(&key).unwrap(), 37)
    }));
    REFUSE_DECODE.set(false);
    assert!(result.is_err());
    assert_eq!(*backing.bytes.borrow(), before);
    let io = backing.io.get();
    assert_eq!((io.writes, io.grows), (0, 0));
    assert!(StableCell::<Record, _>::load(runtime.open_memory(&key).unwrap(), 37).is_ok());
}

#[test]
fn encoding_failure_invalidates_live_owner_without_writing() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let mut cell =
        StableCell::create(runtime.open_memory(&key).unwrap(), Record::new(vec![3; 37])).unwrap();
    let before = backing.bytes.borrow().clone();
    backing.io.set(Io::default());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            cell.set(Record {
                bytes: vec![9; 37],
                refuse_encode: true,
            });
        }))
        .is_err()
    );
    assert_eq!(*backing.bytes.borrow(), before);
    assert_eq!(backing.io.get().writes, 0);
    assert!(matches!(
        runtime.open_memory(&key),
        Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
    ));
    let io = backing.io.get();
    assert!(catch_unwind(AssertUnwindSafe(|| cell.set(Record::new(vec![1; 37])))).is_err());
    assert_eq!(backing.io.get(), io);
}

#[test]
fn every_interrupted_set_write_invalidates_owner_and_preserves_neighbor() {
    let a = StableKey::parse("contract.a.v1").unwrap();
    let b = StableKey::parse("contract.b.v1").unwrap();
    // Discover write boundaries through the public backing, without depending
    // on any particular header, payload offset or publication sequence.
    let probe = ObservedMemory::default();
    let runtime = host(probe.clone());
    let mut cell = StableCell::create(runtime.open_memory(&a).unwrap(), vec![7_u8; 257]).unwrap();
    probe.io.set(Io::default());
    cell.set(vec![8_u8; 257]);
    let writes = probe.io.get().writes;
    assert!(writes > 0);
    for failure in 1..=writes {
        let backing = ObservedMemory::default();
        let runtime = host(backing.clone());
        let mut cell =
            StableCell::create(runtime.open_memory(&a).unwrap(), vec![7_u8; 257]).unwrap();
        drop(StableCell::create(runtime.open_memory(&b).unwrap(), 123_u64).unwrap());
        backing.io.set(Io::default());
        backing.fail_write.set(Some(failure));
        assert!(catch_unwind(AssertUnwindSafe(|| cell.set(vec![8_u8; 257]))).is_err());
        assert_eq!(backing.io.get().writes, failure);
        assert!(matches!(
            runtime.open_memory(&b),
            Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
        ));
        let io = backing.io.get();
        assert!(catch_unwind(AssertUnwindSafe(|| cell.set(vec![9_u8; 257]))).is_err());
        assert_eq!(backing.io.get(), io);
        drop(cell);
        drop(runtime);
        backing.fail_write.set(None);
        let runtime = host(backing);
        assert_eq!(
            *StableCell::<u64, _>::load(runtime.open_memory(&b).unwrap(), 8)
                .unwrap()
                .get(),
            123
        );
    }
}

#[test]
fn later_growth_refusal_invalidates_the_live_owner() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let mut cell = StableCell::create(runtime.open_memory(&key).unwrap(), vec![3_u8]).unwrap();
    let exceeding_capacity = backing.bytes.borrow().len() + 1;
    backing.refuse_growth.set(true);
    backing.io.set(Io::default());
    assert!(catch_unwind(AssertUnwindSafe(|| cell.set(vec![4_u8; exceeding_capacity]))).is_err());
    assert!(backing.io.get().grows > 0);
    assert!(matches!(
        runtime.open_memory(&key),
        Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
    ));
    let io = backing.io.get();
    assert!(catch_unwind(AssertUnwindSafe(|| cell.set(vec![9_u8]))).is_err());
    assert_eq!(backing.io.get(), io);
}

#[test]
fn interrupted_load_reads_poison_owner_and_preserve_bytes() {
    let key = StableKey::parse("contract.a.v1").unwrap();
    let prepare = || {
        let backing = ObservedMemory::default();
        let runtime = host(backing.clone());
        drop(StableCell::create(runtime.open_memory(&key).unwrap(), vec![7_u8; 70_000]).unwrap());
        (backing, runtime)
    };
    let (probe, runtime) = prepare();
    probe.io.set(Io::default());
    drop(StableCell::<Vec<u8>, _>::load(runtime.open_memory(&key).unwrap(), 70_000).unwrap());
    let reads = probe.io.get().reads;
    assert!(reads > 0);
    for failure in 1..=reads {
        let (backing, runtime) = prepare();
        let memory = runtime.open_memory(&key).unwrap();
        let before = backing.bytes.borrow().clone();
        backing.io.set(Io::default());
        backing.fail_read.set(Some(failure));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                StableCell::<Vec<u8>, _>::load(memory, 70_000)
            }))
            .is_err()
        );
        assert_eq!(backing.io.get().reads, failure);
        assert_eq!(*backing.bytes.borrow(), before);
        assert!(matches!(
            runtime.open_memory(&key),
            Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
        ));
    }
}

#[test]
fn interrupted_creation_and_reset_writes_poison_the_owner() {
    let a = StableKey::parse("contract.a.v1").unwrap();
    let b = StableKey::parse("contract.b.v1").unwrap();
    for reset in [false, true] {
        let prepare = || {
            let backing = ObservedMemory::default();
            let runtime = host(backing.clone());
            drop(StableCell::create(runtime.open_memory(&b).unwrap(), 13_u64).unwrap());
            if reset {
                drop(StableCell::create(runtime.open_memory(&a).unwrap(), vec![2_u8; 17]).unwrap());
            }
            (backing, runtime)
        };
        let mutate = |runtime: &MemoryRuntime<ObservedMemory>| {
            let memory = runtime.open_memory(&a).unwrap();
            let result = if reset {
                StableCell::reset(memory, vec![8_u8; 70_000])
            } else {
                StableCell::create(memory, vec![8_u8; 70_000])
            };
            drop(result.unwrap());
        };
        let (probe, runtime) = prepare();
        probe.io.set(Io::default());
        mutate(&runtime);
        let writes = probe.io.get().writes;
        assert!(writes > 0);
        for failure in 1..=writes {
            let (backing, runtime) = prepare();
            backing.io.set(Io::default());
            backing.fail_write.set(Some(failure));
            assert!(catch_unwind(AssertUnwindSafe(|| mutate(&runtime))).is_err());
            assert_eq!(backing.io.get().writes, failure);
            assert!(matches!(
                runtime.open_memory(&b),
                Err(RuntimeOpenError::Directory(DirectoryError::Poisoned))
            ));
        }
    }
}

#[test]
fn initial_growth_refusal_returns_error_without_publishing_cell_bytes() {
    let backing = ObservedMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let memory = runtime.open_memory(&key).unwrap();
    let before = backing.bytes.borrow().clone();
    backing.refuse_growth.set(true);
    backing.io.set(Io::default());
    assert!(StableCell::create(memory, 33_u64).is_err());
    assert!(backing.io.get().grows > 0);
    assert_eq!(*backing.bytes.borrow(), before);
}

#[test]
fn empty_and_foreign_allocations_are_never_initialized_by_load() {
    for nonempty in [false, true] {
        let backing = ObservedMemory::default();
        let runtime = host(backing.clone());
        let key = StableKey::parse("contract.a.v1").unwrap();
        let memory = runtime.open_memory(&key).unwrap();
        if nonempty {
            assert_eq!(memory.grow(1), Ok(0));
            memory.write(0, &[0; 64]);
        }
        let before = backing.bytes.borrow().clone();
        let decodes = DECODE_CALLS.get();
        backing.io.set(Io::default());
        assert!(StableCell::<Record, _>::load(memory, usize::MAX).is_err());
        assert_eq!(DECODE_CALLS.get(), decodes);
        assert_eq!(*backing.bytes.borrow(), before);
        let io = backing.io.get();
        assert_eq!((io.writes, io.grows), (0, 0));
        assert!(runtime.open_memory(&key).is_ok());
    }
}

#[test]
fn replacements_match_owned_values_across_growth_shrink_and_cold_reopen() {
    let backing = VectorMemory::default();
    let key = StableKey::parse("contract.a.v1").unwrap();
    let mut runtime = host(backing.clone());
    let mut cell =
        StableCell::create(runtime.open_memory(&key).unwrap(), Vec::<u8>::new()).unwrap();
    let mut expected = Vec::new();
    for (generation, length) in [0, 1, 17, 4096, 65_535, 65_536, 65_537, 131_073, 3, 0]
        .into_iter()
        .enumerate()
    {
        let generation = u8::try_from(generation).unwrap();
        let next: Vec<u8> = (0_u8..=255)
            .cycle()
            .take(length)
            .map(|byte| byte.wrapping_add(generation))
            .collect();
        assert_eq!(cell.set(next.clone()), expected);
        expected = next;
        assert_eq!(cell.get(), &expected);
        drop(cell);
        drop(runtime);
        let before = backing.borrow().clone();
        runtime = host(backing.clone());
        cell = StableCell::load(runtime.open_memory(&key).unwrap(), expected.len()).unwrap();
        assert_eq!(cell.get(), &expected);
        assert_eq!(*backing.borrow(), before);
    }
}

#[test]
fn refused_admission_preserves_bytes_and_releases_its_lease() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let key = StableKey::parse("contract.a.v1").unwrap();
    let cell = StableCell::create(runtime.open_memory(&key).unwrap(), vec![23_u8; 257]).unwrap();
    assert!(runtime.open_memory(&key).is_err());
    drop(cell);
    let before = backing.borrow().clone();
    assert!(StableCell::<Vec<u8>>::load(runtime.open_memory(&key).unwrap(), 256).is_err());
    assert_eq!(*backing.borrow(), before);
    assert!(StableCell::create(runtime.open_memory(&key).unwrap(), vec![0_u8]).is_err());
    assert_eq!(*backing.borrow(), before);
    let cell = StableCell::<Vec<u8>>::load(runtime.open_memory(&key).unwrap(), 257).unwrap();
    assert_eq!(cell.get(), &vec![23; 257]);
}

#[test]
fn explicit_reset_does_not_change_a_neighboring_cell() {
    let backing = VectorMemory::default();
    let runtime = host(backing.clone());
    let a = StableKey::parse("contract.a.v1").unwrap();
    let b = StableKey::parse("contract.b.v1").unwrap();
    drop(StableCell::create(runtime.open_memory(&a).unwrap(), 41_u64).unwrap());
    drop(StableCell::create(runtime.open_memory(&b).unwrap(), vec![91_u8; 70_000]).unwrap());
    drop(StableCell::reset(runtime.open_memory(&a).unwrap(), 99_u64).unwrap());
    drop(runtime);
    let runtime = host(backing);
    assert_eq!(
        *StableCell::<u64>::load(runtime.open_memory(&a).unwrap(), 8)
            .unwrap()
            .get(),
        99
    );
    assert_eq!(
        StableCell::<Vec<u8>>::load(runtime.open_memory(&b).unwrap(), 70_000)
            .unwrap()
            .get(),
        &vec![91; 70_000]
    );
}
