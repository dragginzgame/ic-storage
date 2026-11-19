//! Public allocation admission and custody regression tests.
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
fn public_runtime_retains_omitted_names_and_requires_current_admission() {
    let memory = VectorMemory::default();
    let mut host = MemoryRuntime::new(memory.clone()).unwrap();
    let names: Vec<_> = (0..4096).map(|n| format!("app.rows{n}.v1")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    host.bootstrap(&source(&names), &grants(), &GenericAllocationPolicy)
        .unwrap();
    let key = StableKey::parse("app.rows4095.v1").unwrap();
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
        &source(&["app.rows0.v1"]),
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
    assert!(matches!(
        host.bootstrap(&invalid, &grants(), &GenericAllocationPolicy),
        Err(RuntimeBootstrapError::Admission(
            AdmissionError::Unauthorized
        ))
    ));
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
    // Deliberately violate raw custody to qualify the checked opening boundary.
    memory.borrow_mut()[0] ^= 1;
    assert!(
        host.open_memory(&StableKey::parse("app.two.v1").unwrap())
            .is_err()
    );
    assert_eq!(rows.grow(0), Err(RuntimeGrowError::Poisoned));
    assert_eq!(rows.grow(1), Err(RuntimeGrowError::Poisoned));
}

#[test]
fn default_allocation_limit_refuses_4097_names_without_writes() {
    let backing = VectorMemory::default();
    let mut host = MemoryRuntime::new(backing.clone()).unwrap();
    let names: Vec<_> = (0..4097).map(|n| format!("app.rows{n}.v1")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    let before = backing.borrow().clone();
    assert!(matches!(
        host.bootstrap(&source(&names), &grants(), &GenericAllocationPolicy),
        Err(RuntimeBootstrapError::Admission(AdmissionError::Limit))
    ));
    assert_eq!(*backing.borrow(), before);
}

#[test]
fn old_directory_is_refused_without_resetting_backing() {
    let backing = VectorMemory::default();
    assert_eq!(Memory::grow(&backing, 1), 0);
    for magic in [b"ICROOTR1", b"ICROOTS1"] {
        Memory::write(&backing, 0, magic);
        let before = backing.borrow().clone();
        assert!(matches!(
            MemoryRuntime::new(backing.clone()),
            Err(RuntimeConstructionError::Directory(
                DirectoryError::Invalid("root marker or reserved bytes")
            ))
        ));
        assert_eq!(*backing.borrow(), before);
    }
}
