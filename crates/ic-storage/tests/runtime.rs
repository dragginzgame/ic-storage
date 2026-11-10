use ic_storage::*;
fn source(keys: &[&str]) -> SealedDeclarationSnapshot {
    SealedDeclarationSnapshot::new(
        keys.iter()
            .map(|key| MemoryRequest::new("app", key).unwrap())
            .collect(),
    )
    .unwrap()
}
fn grants() -> AllocationGrants {
    AllocationGrants::new(vec![MemoryAuthority::new("app", "app.").unwrap()]).unwrap()
}

#[test]
fn diagnostics_are_read_only_and_account_for_retained_data_metadata_and_unassigned_pages() {
    let backing = VectorMemory::default();
    let mut host = MemoryRuntime::new(backing.clone()).unwrap();
    let before = backing.borrow().clone();
    let initial = host.diagnostics().unwrap();
    assert_eq!(initial.allocations, 0);
    assert_eq!(initial.admitted_allocations, 0);
    assert_eq!(*backing.borrow(), before);
    host.bootstrap(
        &source(&["app.a.v1", "app.b.v1"]),
        &grants(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    let a = host
        .open_memory(&StableKey::parse("app.a.v1").unwrap())
        .unwrap();
    let b = host
        .open_memory(&StableKey::parse("app.b.v1").unwrap())
        .unwrap();
    a.grow(2).unwrap();
    b.grow(3).unwrap();
    let before = backing.borrow().clone();
    let report = host.diagnostics().unwrap();
    assert_eq!(report.allocations, 2);
    assert_eq!(report.admitted_allocations, 2);
    assert_eq!(report.live_leases, 2);
    assert_eq!(report.data_pages, 5);
    assert_eq!(report.extents, 2);
    assert!(report.metadata_pages >= 2);
    assert!(report.directory_bytes > 0);
    assert_eq!(
        report.physical_pages,
        1 + report.data_pages + report.metadata_pages + report.unassigned_pages
    );
    assert_eq!(*backing.borrow(), before);
    drop(a);
    drop(b);
    drop(host);
    let mut host = MemoryRuntime::new(backing.clone()).unwrap();
    host.bootstrap(&source(&["app.a.v1"]), &grants(), &GenericAllocationPolicy)
        .unwrap();
    let reopened = host.diagnostics().unwrap();
    assert_eq!(reopened.generation, report.generation);
    assert_eq!(reopened.allocations, 2);
    assert_eq!(reopened.admitted_allocations, 1);
    assert_eq!(reopened.live_leases, 0);
    assert_eq!(reopened.data_pages, report.data_pages);
    assert_eq!(reopened.metadata_pages, report.metadata_pages);
    assert_eq!(reopened.unassigned_pages, report.unassigned_pages);
    // Corruption is a checked-boundary failure, not a successful cached report.
    backing.borrow_mut()[0] ^= 1;
    assert!(host.diagnostics().is_err());
    assert!(matches!(host.diagnostics(), Err(DirectoryError::Poisoned)));
}

#[test]
fn public_runtime_retains_omitted_names_and_requires_current_admission() {
    let memory = VectorMemory::default();
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    let names: Vec<_> = (0..1024).map(|n| format!("app.rows{n}.v1")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    host.bootstrap(&source(&names), &grants(), &GenericAllocationPolicy)
        .unwrap();
    let key = StableKey::parse("app.rows999.v1").unwrap();
    let id = host.allocation_id(&key).unwrap();
    assert!(id.get() > 255);
    let rows = host.open_memory(&key).unwrap();
    rows.grow(1).unwrap();
    rows.write(17, b"retained");
    drop(host);
    assert!(matches!(
        MemoryRuntime::new(memory.clone()),
        Err(RuntimeConstructionError::BackingAlreadyClaimed)
    ));
    drop(rows);
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    host.bootstrap(
        &source(&["app.other.v1"]),
        &grants(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    assert_eq!(host.allocation_id(&key), Some(id));
    assert!(matches!(
        host.open_memory(&key),
        Err(RuntimeOpenError::NotAdmitted(_))
    ));
    drop(host);
    let mut host = MemoryRuntime::new(memory).unwrap();
    host.bootstrap(
        &source(&[key.as_str()]),
        &grants(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    let rows = host.open_memory(&key).unwrap();
    let mut bytes = [0; 8];
    rows.read(17, &mut bytes);
    assert_eq!(&bytes, b"retained");
}

struct Select;
impl RuntimeBootstrapPolicy for Select {
    type Error = &'static str;
    fn validate_allocation(&self, _: &StableKey) -> Result<(), Self::Error> {
        Ok(())
    }
    fn runtime_bootstrap_identity(&self) -> PolicyIdentity {
        PolicyIdentity::new([7; 32])
    }
    fn prepare_bootstrap(&self, admission: &mut BootstrapAdmission<'_>) -> Result<(), Self::Error> {
        admission
            .include_historical("app", "app.old.v1")
            .map_err(|_| "selection")
    }
}
#[test]
fn historical_selection_warm_binding_and_namespace_refusal() {
    let memory = VectorMemory::default();
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    let invalid =
        SealedDeclarationSnapshot::new(vec![MemoryRequest::new("other", "app.old.v1").unwrap()])
            .unwrap();
    let before = memory.borrow().clone();
    assert!(
        host.bootstrap(&invalid, &grants(), &GenericAllocationPolicy)
            .is_err()
    );
    assert_eq!(*memory.borrow(), before);
    host.bootstrap(
        &source(&["app.old.v1"]),
        &grants(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    drop(host);
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    let empty = source(&[]);
    host.bootstrap(&empty, &grants(), &Select).unwrap();
    let before = memory.borrow().clone();
    host.verify_bootstrap(&empty, &grants(), &Select).unwrap();
    assert!(matches!(
        host.bootstrap(&empty, &grants(), &GenericAllocationPolicy),
        Err(RuntimeBootstrapError::BindingMismatch)
    ));
    assert_eq!(*memory.borrow(), before);
    assert!(
        host.open_memory(&StableKey::parse("app.old.v1").unwrap())
            .is_ok()
    );
}

#[test]
fn fallible_growth_reports_overflow_and_poisoning_without_panicking() {
    let memory = VectorMemory::default();
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    host.bootstrap(
        &source(&["app.one.v1", "app.two.v1"]),
        &grants(),
        &GenericAllocationPolicy,
    )
    .unwrap();
    let rows = host
        .open_memory(&StableKey::parse("app.one.v1").unwrap())
        .unwrap();
    rows.grow(1).unwrap();
    let before = memory.borrow().clone();
    assert_eq!(
        rows.grow(u64::MAX),
        Err(RuntimeGrowError::ArithmeticOverflow)
    );
    assert_eq!(*memory.borrow(), before);
    // Deliberately violate raw custody to qualify explicit verification.
    memory.borrow_mut()[0] ^= 1;
    let other = host
        .open_memory(&StableKey::parse("app.two.v1").unwrap())
        .unwrap();
    assert!(host.memory_allocations().is_err());
    assert_eq!(other.grow(0), Err(RuntimeGrowError::Poisoned));
    assert_eq!(rows.grow(0), Err(RuntimeGrowError::Poisoned));
    assert_eq!(rows.grow(1), Err(RuntimeGrowError::Poisoned));
}

#[test]
fn unexplained_native_backing_growth_is_rejected_without_reset() {
    let backing = VectorMemory::default();
    drop(MemoryRuntime::new(backing.clone()).unwrap());
    backing.grow(2);
    let before = backing.borrow().clone();
    assert!(MemoryRuntime::new(backing.clone()).is_err());
    assert_eq!(*backing.borrow(), before);
}
