use super::*;
use crate::{BackingIdentity, MemoryRuntime, VectorMemory, WASM_PAGE_SIZE_BYTES as PAGE};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn key(index: usize) -> StableKey {
    StableKey::parse(format!("lease.store{index}.v1")).unwrap()
}

#[test]
fn sparse_leases_exceed_256_and_retain_custody_after_owner_drop() {
    let memory = VectorMemory::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&(0..1_024).map(key).collect()).unwrap();
    let handles: Vec<_> = (0..1_024).map(|i| owner.lease(&key(i)).unwrap()).collect();
    assert!(handles.iter().any(|handle| handle.id.0 > 255));
    assert!(matches!(owner.lease(&key(0)), Err(Error::AlreadyOpen)));
    assert!(matches!(owner.lease(&key(9_999)), Err(Error::Missing)));
    assert!(matches!(
        MemoryRuntime::new(memory.clone()),
        Err(crate::RuntimeConstructionError::BackingAlreadyClaimed)
    ));
    drop(owner);
    assert!(matches!(
        Owner::init(memory.clone(), StorageConfig::default()),
        Err(Error::AlreadyClaimed)
    ));
    let last = handles.into_iter().last().unwrap();
    assert_eq!(last.try_grow(1).unwrap(), 0);
    last.write(12, b"detached");
    drop(last);
    let owner = Owner::init(memory, StorageConfig::default()).unwrap();
    let last = owner.lease(&key(1_023)).unwrap();
    let mut bytes = [0; 8];
    last.read(12, &mut bytes);
    assert_eq!(&bytes, b"detached");
    drop(last);
    assert!(owner.lease(&key(1_023)).is_ok());
}

#[test]
fn interleaved_extents_translate_cross_page_io_and_preflight_bounds() {
    let memory = VectorMemory::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0), key(1)].into()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    let b = owner.lease(&key(1)).unwrap();
    assert_eq!(a.try_grow(1).unwrap(), 0);
    assert_eq!(b.try_grow(1).unwrap(), 0);
    b.write(0, b"other allocation");
    assert_eq!(a.try_grow(2).unwrap(), 1);
    let payload: Vec<_> = (0..PAGE + 32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    a.write(PAGE - 8, &payload);
    let mut actual = vec![0; payload.len()];
    a.read(PAGE - 8, &mut actual);
    assert_eq!(actual, payload);
    let mut other = [0; 16];
    b.read(0, &mut other);
    assert_eq!(&other, b"other allocation");
    let before = memory.borrow().clone();
    for (offset, bytes) in [(3 * PAGE - 1, &[1, 2][..]), (u64::MAX, &[1][..])] {
        assert!(catch_unwind(AssertUnwindSafe(|| a.write(offset, bytes))).is_err());
        assert_eq!(*memory.borrow(), before);
    }
    let mut target = [99; 2];
    assert!(catch_unwind(AssertUnwindSafe(|| a.read(3 * PAGE - 1, &mut target))).is_err());
    assert_eq!(target, [99; 2]);
    // Caller bounds errors do not poison a valid owner.
    a.write(3 * PAGE, &[]);
    a.read(3 * PAGE, &mut []);
    assert_eq!(a.try_grow(0).unwrap(), 3);
    assert_eq!(*memory.borrow(), before);
    assert!(catch_unwind(AssertUnwindSafe(|| a.read(3 * PAGE + 1, &mut []))).is_err());
}

#[test]
fn admitted_indices_survive_directory_growth_and_interleaved_extents() {
    let memory = VectorMemory::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(900)].into()).unwrap();
    let first = owner.lease(&key(900)).unwrap();
    first.try_grow(1).unwrap();
    first.write(0, b"first");
    owner.bind(&(0..512).map(key).collect()).unwrap();
    let second = owner.lease(&key(0)).unwrap();
    second.try_grow(2).unwrap();
    first.try_grow(2).unwrap();
    first.write(PAGE, b"later");
    assert_eq!(first.size(), 3);
    assert_eq!(second.size(), 2);
    drop(owner);
    let mut bytes = [0; 5];
    first.read(0, &mut bytes);
    assert_eq!(&bytes, b"first");
    first.read(PAGE, &mut bytes);
    assert_eq!(&bytes, b"later");
    drop((first, second));
    let owner = Owner::init(memory, StorageConfig::default()).unwrap();
    assert_eq!(owner.lease(&key(900)).unwrap().size(), 3);
    assert_eq!(owner.lease(&key(0)).unwrap().size(), 2);
}

#[derive(Clone, Default)]
struct Faults {
    memory: VectorMemory,
    fail_io: Rc<Cell<bool>>,
    refuse_growth: Rc<Cell<bool>>,
    reads: Rc<Cell<usize>>,
    fail_read_at: Rc<Cell<Option<usize>>>,
}

impl MemoryBacking for Faults {
    fn backing_identity(&self) -> BackingIdentity {
        self.memory.backing_identity()
    }
}

impl Memory for Faults {
    fn size(&self) -> u64 {
        self.memory.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        if self.refuse_growth.get() {
            -1
        } else {
            self.memory.grow(pages)
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.reads.set(self.reads.get() + 1);
        assert_ne!(
            self.fail_read_at.get(),
            Some(self.reads.get()),
            "injected chunk failure"
        );
        assert!(!self.fail_io.get(), "injected read failure");
        self.memory.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        assert!(!self.fail_io.get(), "injected write failure");
        self.memory.write(offset, bytes);
    }
}

#[test]
fn uninitialized_reads_admit_full_range_and_poison_partial_native_failures() {
    use std::mem::MaybeUninit;
    #[expect(
        unsafe_code,
        reason = "Test-owned independent buffers satisfy the raw-read contract"
    )]
    fn read<'a>(
        lease: &Lease<Faults>,
        offset: u64,
        bytes: &'a mut [MaybeUninit<u8>],
    ) -> &'a mut [u8] {
        // SAFETY: the test supplies an exclusive independent buffer. Only
        // successful whole reads reach the initialized-slice conversion.
        unsafe {
            lease.read_unsafe(offset, bytes.as_mut_ptr().cast(), bytes.len());
            std::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast(), bytes.len())
        }
    }
    let memory = Faults::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0), key(1)].into()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    let b = owner.lease(&key(1)).unwrap();
    a.try_grow(8).unwrap();
    b.try_grow(1).unwrap();
    a.try_grow(1).unwrap();
    a.write(8 * PAGE - 2, &[11, 12, 13, 14]);
    let mut bytes = [MaybeUninit::uninit(); 4];
    memory.reads.set(0);
    assert_eq!(read(&a, 8 * PAGE - 2, &mut bytes), &[11, 12, 13, 14]);
    assert_eq!(memory.reads.get(), 2);
    memory.reads.set(0);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || read(&a, 2 * 8 * PAGE - 2, &mut bytes).len()
        ))
        .is_err()
    );
    assert_eq!(
        memory.reads.get(),
        0,
        "reject the complete range before backing IO"
    );
    assert_eq!(read(&a, 9 * PAGE, &mut []), [0_u8; 0]);
    memory.fail_read_at.set(Some(2));
    assert!(catch_unwind(AssertUnwindSafe(|| read(&a, 8 * PAGE - 2, &mut bytes).len())).is_err());
    assert_eq!(memory.reads.get(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| read(&b, 0, &mut bytes).len())).is_err());
    assert_eq!(
        memory.reads.get(),
        2,
        "partial read poisons all leases before further IO"
    );
}

#[test]
fn bucket_spans_split_io_and_rebuild_on_reopen() {
    let memory = Faults::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0), key(1)].into()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    let b = owner.lease(&key(1)).unwrap();
    a.try_grow(8).unwrap();
    b.try_grow(1).unwrap();
    b.write(0, b"neighbour");
    a.try_grow(2).unwrap();
    let payload = vec![17; usize::try_from(10 * PAGE - 20).unwrap()];
    a.write(10, &payload);
    drop(a);
    drop(b);
    drop(owner);
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    assert_eq!(a.size(), 10);
    memory.reads.set(0);
    let mut result = vec![0; payload.len()];
    a.read(10, &mut result);
    assert_eq!(result, payload);
    assert_eq!(memory.reads.get(), 2, "one backing read per bucket");
    let b = owner.lease(&key(1)).unwrap();
    let mut neighbour = [0; 9];
    b.read(0, &mut neighbour);
    assert_eq!(&neighbour, b"neighbour");
}

#[test]
fn data_io_trusts_custody_and_growth_refusal_preserves_handles() {
    let memory = Faults::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0)].into()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    assert_eq!(a.try_grow(1).unwrap(), 0);
    memory.reads.set(0);
    a.write(0, &[11]);
    assert_eq!(memory.reads.get(), 0);
    let mut value = [0];
    a.read(0, &mut value);
    assert_eq!(value, [11]);
    assert_eq!(memory.reads.get(), 1);
    memory.refuse_growth.set(true);
    let before = memory.memory.borrow().clone();
    assert!(matches!(a.try_grow(8), Err(Error::Growth)));
    assert_eq!(a.size(), 1);
    assert_eq!(*memory.memory.borrow(), before);
    memory.refuse_growth.set(false);
    assert_eq!(a.try_grow(1).unwrap(), 1);
}

#[test]
fn backing_panics_and_boundary_corruption_poison_all_live_leases() {
    for read_failure in [false, true] {
        let memory = Faults::default();
        let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
        owner.bind(&[key(0), key(1)].into()).unwrap();
        let a = owner.lease(&key(0)).unwrap();
        let b = owner.lease(&key(1)).unwrap();
        a.try_grow(1).unwrap();
        memory.fail_io.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if read_failure {
                    a.read(0, &mut [0]);
                } else {
                    a.write(0, &[0]);
                }
            }))
            .is_err()
        );
        memory.fail_io.set(false);
        assert!(matches!(b.try_grow(1), Err(Error::Poisoned)));
        assert!(matches!(owner.bind(&[key(2)].into()), Err(Error::Poisoned)));
        assert!(catch_unwind(AssertUnwindSafe(|| a.size())).is_err());
    }
    let memory = VectorMemory::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0), key(1)].into()).unwrap();
    let a = owner.lease(&key(0)).unwrap();
    // Deliberate fixture corruption at a checked lease-opening boundary.
    memory.borrow_mut()[0] ^= 1;
    assert!(owner.lease(&key(1)).is_err());
    assert!(matches!(a.try_grow(1), Err(Error::Poisoned)));
}

#[test]
fn cached_collection_reads_still_observe_poisoning_from_another_lease() {
    let memory = Faults::default();
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    owner.bind(&[key(0), key(1)].into()).unwrap();
    let handle = crate::RuntimeMemory {
        lease: owner.lease(&key(0)).unwrap(),
    };
    let mut map = crate::adapter::StableBTreeMap::<u64, u64, _>::create(handle).unwrap();
    map.insert(1, 2);
    map.insert(2, 3);
    assert_eq!(map.get(&1), Some(2));
    let mut iterator = map.iter();
    assert_eq!(iterator.next().unwrap().into_pair(), (1, 2));
    let other = owner.lease(&key(1)).unwrap();
    other.try_grow(1).unwrap();
    memory.fail_io.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| other.write(0, &[1]))).is_err());
    memory.fail_io.set(false);
    assert!(catch_unwind(AssertUnwindSafe(|| map.get(&1))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| iterator.next())).is_err());
}

#[test]
fn construction_failures_release_claim_without_resetting_backing() {
    let memory = Faults::default();
    memory.refuse_growth.set(true);
    assert!(matches!(
        Owner::init(memory.clone(), StorageConfig::default()),
        Err(Error::Growth)
    ));
    memory.refuse_growth.set(false);
    let owner = Owner::init(memory.clone(), StorageConfig::default()).unwrap();
    drop(owner);
    memory.fail_io.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| Owner::init(
            memory.clone(),
            StorageConfig::default()
        )))
        .is_err()
    );
    memory.fail_io.set(false);
    drop(Owner::init(memory.clone(), StorageConfig::default()).unwrap());
    let before = memory.memory.borrow().clone();
    drop(Owner::init(memory.clone(), StorageConfig::default()).unwrap());
    assert_eq!(*memory.memory.borrow(), before);
}
