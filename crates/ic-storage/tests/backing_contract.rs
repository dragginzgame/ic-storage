//! Public allocation acceptance against byte models and injected backing failures.
//! No private format constants or implementation helpers are used here.
use ic_storage::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
const PAGE: usize = 65_536;

#[derive(Clone, Default)]
struct Observed {
    bytes: VectorMemory,
    writes: Rc<RefCell<Vec<(u64, usize)>>>,
    reads: Rc<Cell<usize>>,
    read_bytes: Rc<Cell<usize>>,
    fail: Rc<Cell<Option<(usize, usize)>>>,
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
        if self.refuse_growth.get() {
            -1
        } else {
            self.bytes.grow(pages)
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.reads.set(self.reads.get() + 1);
        self.read_bytes.set(self.read_bytes.get() + bytes.len());
        self.bytes.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        let index = self.writes.borrow().len();
        self.writes.borrow_mut().push((offset, bytes.len()));
        if let Some((at, prefix)) = self.fail.get()
            && at == index
        {
            self.bytes.write(offset, &bytes[..prefix.min(bytes.len())]);
            panic!("injected publication failure");
        }
        self.bytes.write(offset, bytes);
    }
}
fn key(index: usize) -> StableKey {
    StableKey::parse(format!("backing.rows{index}.v1")).unwrap()
}
fn bootstrap<M: Memory>(host: &mut MemoryRuntime<M>, names: usize) {
    let declarations = SealedDeclarationSnapshot::new(
        (0..names)
            .map(|i| MemoryRequest::new("backing", key(i).as_str()).unwrap())
            .collect(),
    )
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("backing", "backing.").unwrap()]).unwrap();
    host.bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
}
fn host(backing: &Observed, names: usize) -> MemoryRuntime<Observed> {
    let mut host = MemoryRuntime::new(backing.clone()).unwrap();
    bootstrap(&mut host, names);
    host
}

#[test]
fn interleaved_growth_matches_independent_byte_models_across_reopen() {
    let backing = Observed::default();
    let mut expected = vec![Vec::<u8>::new(); 4];
    let mut ids = Vec::new();
    for turn in 0..5_usize {
        let host = host(&backing, 4);
        let handles: Vec<_> = (0..4).map(|i| host.open_memory(&key(i)).unwrap()).collect();
        let current: Vec<_> = (0..4)
            .map(|i| host.allocation_id(&key(i)).unwrap())
            .collect();
        if turn == 0 {
            ids = current;
        } else {
            assert_eq!(current, ids);
        }
        for step in 0..12 {
            let index = (step * 3 + turn) % 4;
            let old = expected[index].len();
            let pages = [0, 1, 7, 9][(step + turn) % 4];
            assert_eq!(
                handles[index].grow(pages as u64).unwrap(),
                (old / PAGE) as u64
            );
            expected[index].resize(old + pages * PAGE, 0);
            if pages != 0 {
                let mut appended = vec![1; pages * PAGE];
                handles[index].read(old as u64, &mut appended);
                assert!(appended.iter().all(|&byte| byte == 0));
            }
            if !expected[index].is_empty() {
                let at = ((step * 65_531 + turn * 31) % expected[index].len()).saturating_sub(4);
                let length = (PAGE + 19).min(expected[index].len() - at);
                let bytes: Vec<_> = (0..length)
                    .map(|n| u8::try_from((n + step + turn) % 251).unwrap())
                    .collect();
                handles[index].write(at as u64, &bytes);
                expected[index][at..at + length].copy_from_slice(&bytes);
            }
        }
        for (handle, model) in handles.iter().zip(&expected) {
            let mut actual = vec![0; model.len()];
            handle.read(0, &mut actual);
            assert_eq!(&actual, model);
            let mut untouched = [99; 2];
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || handle.read(model.len().saturating_sub(1) as u64, &mut untouched)
                ))
                .is_err()
            );
            assert_eq!(untouched, [99; 2]);
            handle.read(model.len() as u64, &mut []);
        }
        let report = host.diagnostics().unwrap();
        assert_eq!(
            report.data_pages,
            expected
                .iter()
                .map(|v| (v.len() / PAGE) as u64)
                .sum::<u64>()
        );
        assert_eq!(
            report.physical_pages,
            1 + report.metadata_pages + report.data_pages + report.unassigned_pages
        );
    }
}

#[test]
fn growth_refusal_is_retryable_and_saved_capacity_cannot_expand() {
    let backing = Observed::default();
    let budget = StorageConfig {
        allocations: 1,
        extents: 1,
        pages: 64,
    };
    let mut host = MemoryRuntime::new_with_config(backing.clone(), budget).unwrap();
    bootstrap(&mut host, 1);
    let handle = host.open_memory(&key(0)).unwrap();
    let original = backing.bytes.borrow().clone();
    backing.writes.borrow_mut().clear();
    backing.refuse_growth.set(true);
    assert!(matches!(
        handle.grow(1),
        Err(RuntimeGrowError::BackingRefused { .. })
    ));
    assert_eq!(*backing.bytes.borrow(), original);
    assert!(backing.writes.borrow().is_empty());
    assert_eq!(handle.size(), 0);
    backing.refuse_growth.set(false);
    handle.grow(1).unwrap();
    let physical = backing.size();
    handle.grow(7).unwrap();
    assert_eq!(backing.size(), physical);
    assert_eq!(handle.grow(1), Err(RuntimeGrowError::CapacityExceeded));
    assert_eq!(
        handle.grow(u64::MAX),
        Err(RuntimeGrowError::ArithmeticOverflow)
    );
    drop((handle, host));
    let mut reopened = MemoryRuntime::new(backing.clone()).unwrap();
    let before = backing.bytes.borrow().clone();
    let declarations = SealedDeclarationSnapshot::new(
        (0..2)
            .map(|i| MemoryRequest::new("backing", key(i).as_str()).unwrap())
            .collect(),
    )
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("backing", "backing.").unwrap()]).unwrap();
    assert!(
        reopened
            .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
            .is_err()
    );
    assert_eq!(*backing.bytes.borrow(), before);
}

#[test]
fn admitted_handles_and_io_do_not_rescan_unrelated_metadata() {
    let backing = Observed::default();
    let host = host(&backing, 1024);
    backing.reads.set(0);
    let handle = host.open_memory(&key(1000)).unwrap();
    assert_eq!(handle.size(), 0);
    assert_eq!(backing.reads.get(), 0);
    handle.grow(17).unwrap();
    assert_eq!(backing.reads.get(), 0);
    backing.writes.borrow_mut().clear();
    let data = vec![21; 17 * PAGE];
    handle.write(0, &data);
    assert!(
        backing.writes.borrow().len() <= 3,
        "bulk IO must not become page-by-page IO"
    );
    let mut actual = vec![0; data.len()];
    handle.read(0, &mut actual);
    assert_eq!(actual, data);
    assert!(backing.reads.get() <= 3);
    drop((handle, host));
    backing.reads.set(0);
    backing.read_bytes.set(0);
    backing.writes.borrow_mut().clear();
    let reopened = MemoryRuntime::new(backing.clone()).unwrap();
    assert!(backing.writes.borrow().is_empty());
    assert!(
        backing.read_bytes.get() < backing.bytes.borrow().len() / 2,
        "admission must not read payload or unused controls"
    );
    drop(reopened);
}

#[derive(Clone, Copy)]
enum Mutation {
    Bind,
    Grow,
}
fn prepare() -> (Observed, MemoryRuntime<Observed>) {
    let backing = Observed::default();
    let host = host(&backing, 2);
    let neighbor = host.open_memory(&key(1)).unwrap();
    neighbor.grow(1).unwrap();
    neighbor.write(13, b"neighbor");
    drop((neighbor, host));
    let host = MemoryRuntime::new(backing.clone()).unwrap();
    backing.writes.borrow_mut().clear();
    (backing, host)
}
fn mutate(host: &mut MemoryRuntime<Observed>, mutation: Mutation) {
    match mutation {
        Mutation::Bind => bootstrap(host, 3),
        Mutation::Grow => {
            bootstrap(host, 2);
            host.open_memory(&key(0)).unwrap().grow(9).unwrap();
        }
    }
}
#[test]
fn every_interrupted_metadata_write_reopens_only_a_complete_state() {
    for mutation in [Mutation::Bind, Mutation::Grow] {
        let (probe, mut runtime) = prepare();
        mutate(&mut runtime, mutation);
        let writes = probe.writes.borrow().clone();
        assert_ne!(writes, [] as [(u64, usize); 0]);
        for (at, (_, length)) in writes.into_iter().enumerate() {
            for prefix in [0, 1, length / 2, length] {
                let (backing, mut runtime) = prepare();
                backing.fail.set(Some((at, prefix)));
                assert!(catch_unwind(AssertUnwindSafe(|| mutate(&mut runtime, mutation))).is_err());
                let observed = backing.writes.borrow().len();
                assert!(matches!(
                    runtime.diagnostics(),
                    Err(DirectoryError::Poisoned)
                ));
                assert_eq!(backing.writes.borrow().len(), observed);
                backing.fail.set(None);
                drop(runtime);
                let before = backing.bytes.borrow().clone();
                if let Ok(mut reopened) = MemoryRuntime::new(backing.clone()) {
                    let report = reopened.diagnostics().unwrap();
                    match mutation {
                        Mutation::Bind => assert!([2, 3].contains(&report.allocations)),
                        Mutation::Grow => assert!([1, 10].contains(&report.data_pages)),
                    }
                    bootstrap(&mut reopened, 2);
                    let neighbor = reopened.open_memory(&key(1)).unwrap();
                    let mut bytes = [0; 8];
                    neighbor.read(13, &mut bytes);
                    assert_eq!(&bytes, b"neighbor");
                }
                assert_eq!(
                    *backing.bytes.borrow(),
                    before,
                    "reopening never repairs partial publication"
                );
            }
        }
    }
}
