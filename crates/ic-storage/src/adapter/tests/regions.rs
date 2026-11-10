//! Retained control-layout qualification at the adapter/lease boundary.
#![expect(
    clippy::manual_assert_eq,
    reason = "failed byte checks must not print multi-megabyte backing buffers"
)]
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

const START: u64 = 257;
const END: u64 = 4_194_304;
const PAGE: u64 = 65_536;

#[test]
fn region_growth_error_retains_the_runtime_source() {
    use std::error::Error as _;
    let growth = crate::RuntimeGrowError::BackingRefused {
        additional_pages: 1,
    };
    let error = RegionGrowError::Runtime(growth);
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<crate::RuntimeGrowError>(),
        Some(&growth)
    );
    assert!(
        RegionGrowError::LimitExceeded {
            minimum: 2,
            maximum: 1
        }
        .source()
        .is_none()
    );
}

const fn namespace(family: u8) -> std::ops::RangeInclusive<[u8; 33]> {
    let mut start = [0; 33];
    start[0] = family;
    let mut end = [u8::MAX; 33];
    end[0] = family;
    start..=end
}

// Representative consumer admission, before infallible collection loading.
// Application header codecs/CRCs and recovery policy remain with IcyDB.
fn admit<M: Memory>(view: &BTreeMapRegion<[u8; 33], Vec<u8>, M>) -> Result<(), ()> {
    let mut frame = [0; 8];
    view.read(0, &mut frame);
    if frame != *b"STORMAP1" {
        return Err(());
    }
    let mut after_families = [0; 33];
    after_families[0] = 3;
    if view.get(&[0; 33]).unwrap().as_deref() != Some(b"header")
        || view.keys_range(namespace(0)).unwrap().any(|k| k != [0; 33])
        || view.keys_range(after_families..).unwrap().next().is_some()
    {
        return Err(());
    }
    Ok(())
}

#[test]
fn retained_region_interleaves_namespaces_and_cold_reopens() {
    let backing = VectorMemory::default();
    let runtime = runtime(backing.clone());
    let lease = Rc::new(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    );
    lease.as_ref().grow(START).unwrap();
    for offset in [0, PAGE, (START - 1) * PAGE] {
        lease.write(offset, b"frame");
    }

    let mut retained = BTreeMapRegion::new(lease.clone(), START..END);
    retained.initialize().unwrap();
    retained.insert([0; 33], b"header".to_vec()).unwrap();
    retained.insert([1; 33], vec![9]).unwrap();
    drop(retained);
    let before = backing.borrow().clone();

    let mut application = BTreeMapRegion::new(lease.clone(), START..END);
    let mut checkpoint = BTreeMapRegion::new(lease.clone(), START..END);
    admit(&application).unwrap();
    admit(&checkpoint).unwrap();
    assert!(*backing.borrow() == before);
    let owners = Rc::strong_count(&lease);
    let snapshot = application.keys_range(namespace(1)).unwrap();
    assert_eq!(Rc::strong_count(&lease), owners);
    assert!(application.initialize().is_err());
    assert!(*backing.borrow() == before);

    for number in 0..80_u8 {
        let mut key = [1; 33];
        key[32] = number;
        application.insert(key, vec![number; 40]).unwrap();
        key[0] = 2;
        checkpoint.insert(key, vec![number + 1; 40]).unwrap();
    }
    assert_eq!(snapshot.collect::<Vec<_>>(), vec![[1; 33]]);
    assert_eq!(application.keys_range(namespace(1)).unwrap().count(), 80);
    assert_eq!(checkpoint.keys_range(namespace(2)).unwrap().count(), 80);
    assert_eq!(application.len().unwrap(), 161);
    for number in 0..80_u8 {
        let mut key = [1; 33];
        key[32] = number;
        assert_eq!(application.remove(&key).unwrap(), Some(vec![number; 40]));
        key[0] = 2;
        assert_eq!(checkpoint.get(&key).unwrap(), Some(vec![number + 1; 40]));
    }
    // Commit, boot and startup frames remain disjoint from collection writes.
    for offset in [0, PAGE, (START - 1) * PAGE] {
        let mut frame = [0; 5];
        lease.read(offset, &mut frame);
        assert_eq!(&frame, b"frame");
    }
    drop(lease);
    drop(application);
    assert!(matches!(
        runtime.open_memory(&crate::StableKey::parse("test.map.v1").unwrap()),
        Err(RuntimeOpenError::AlreadyOpen { .. })
    ));
    drop(checkpoint);
    drop(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    );
    drop(runtime);

    // Cold reopen through the ordinary runtime bootstrap models the retained
    // allocation handoff across an upgrade, with no second live runtime.
    let runtime = super::runtime(backing.clone());
    let before = backing.borrow().clone();
    let lease = Rc::new(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    );
    let view = BTreeMapRegion::new(lease.clone(), START..END);
    admit(&view).unwrap();
    assert_eq!(view.len().unwrap(), 81);
    assert_eq!(view.keys_range(namespace(1)).unwrap().count(), 0);
    for number in 0..80_u8 {
        let mut key = [1; 33];
        key[0] = 2;
        key[32] = number;
        assert_eq!(view.get(&key).unwrap(), Some(vec![number + 1; 40]));
    }
    assert!(*backing.borrow() == before);
    drop(view);
    let reopened = BTreeMapRegion::<[u8; 33], Vec<u8>, _>::new(lease, START..END);
    assert_eq!(reopened.len().unwrap(), 81);
    assert!(reopened.keys_range(..).unwrap().is_sorted());
    assert!(*backing.borrow() == before);
}

#[test]
fn retained_region_admission_refuses_corruption_without_rewriting() {
    for corruption in 0..5 {
        let backing = VectorMemory::default();
        let runtime = runtime(backing.clone());
        let lease = Rc::new(
            runtime
                .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
                .unwrap(),
        );
        let mut view = BTreeMapRegion::new(lease.clone(), 0..8);
        assert_eq!(view.allocated_pages(), 0);
        view.initialize().unwrap();
        view.insert([0; 33], b"header".to_vec()).unwrap();
        match corruption {
            0..=1 => {
                let offset = [0, 3][corruption];
                lease.write(offset, &[0xff]);
            }
            2 | 3 => {
                let mut key = [0; 33];
                key[0] = if corruption == 2 { 0 } else { 3 };
                key[32] = 1;
                view.insert(key, vec![0]).unwrap();
            }
            _ => {
                view.insert([0; 33], vec![0xff]).unwrap();
            }
        }
        let before = backing.borrow().clone();
        assert!(admit(&view).is_err());
        assert!(view.initialize().is_err());
        assert!(*backing.borrow() == before);
        drop(view);
        assert!(matches!(
            runtime.open_memory(&crate::StableKey::parse("test.map.v1").unwrap()),
            Err(RuntimeOpenError::AlreadyOpen { .. })
        ));
        drop(lease);
        drop(
            runtime
                .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
                .unwrap(),
        );
    }
}

#[test]
fn retained_region_bounds_and_growth_refusal_preserve_bytes_and_leases() {
    let memory = ControlledMemory {
        backing: VectorMemory::default(),
        refuse_growth: Rc::new(Cell::new(false)),
        growth_calls: Rc::new(Cell::new(0)),
    };
    let runtime = runtime(memory.clone());
    let lease = Rc::new(
        runtime
            .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    );
    lease.as_ref().grow(1).unwrap();
    lease.write(0, b"frame");
    let mut view = BTreeMapRegion::<u64, Vec<u8>, _>::new(lease.clone(), START..START + 128);
    let before = memory.backing.borrow().clone();
    let allocations = runtime.memory_allocations().unwrap();
    assert_eq!(view.allocated_pages(), 0);
    view.read(0, &mut []);
    view.reserve_pages(0).unwrap();
    // Even an absent region rejects overgrowth before allocating the gap.
    assert_eq!(
        view.reserve_pages(129),
        Err(RegionGrowError::LimitExceeded {
            minimum: 129,
            maximum: 128
        })
    );
    assert!(catch_unwind(AssertUnwindSafe(|| view.read(0, &mut [0]))).is_err());
    assert!(*memory.backing.borrow() == before);
    assert_eq!(runtime.memory_allocations().unwrap(), allocations);

    memory.refuse_growth.set(true);
    assert!(matches!(
        view.reserve_pages(1),
        Err(RegionGrowError::Runtime(
            crate::RuntimeGrowError::BackingRefused { .. }
        ))
    ));
    assert!(*memory.backing.borrow() == before);
    assert_eq!(runtime.memory_allocations().unwrap(), allocations);
    memory.refuse_growth.set(false);
    view.initialize().unwrap();
    assert!(view.is_empty().unwrap());
    view.insert(7, vec![11]).unwrap();
    view.reserve_pages(127).unwrap();
    assert_eq!(view.allocated_pages(), 127);
    let before = memory.backing.borrow().clone();
    let allocations = runtime.memory_allocations().unwrap();
    let growth_calls = memory.growth_calls.get();
    view.reserve_pages(0).unwrap();
    view.reserve_pages(127).unwrap();
    assert_eq!(memory.growth_calls.get(), growth_calls);
    memory.refuse_growth.set(true);
    assert!(matches!(
        view.reserve_pages(128),
        Err(RegionGrowError::Runtime(
            crate::RuntimeGrowError::BackingRefused { .. }
        ))
    ));
    assert!(matches!(
        view.reserve_pages(u64::MAX),
        Err(RegionGrowError::LimitExceeded { .. })
    ));
    assert!(*memory.backing.borrow() == before);
    assert_eq!(runtime.memory_allocations().unwrap(), allocations);
    assert_eq!(view.get(&7).unwrap(), Some(vec![11]));
    assert_eq!(view.allocated_pages(), 127);
    memory.refuse_growth.set(false);
    view.reserve_pages(128).unwrap();
    view.insert(8, vec![12]).unwrap();

    // The containing allocation also has live suffix frames. Neither public
    // inspection nor private collection IO can cross the region's end.
    lease.as_ref().grow(1).unwrap();
    let end = 128 * PAGE;
    lease.write((START + 128) * PAGE, b"suffix");
    view.read(end, &mut []);
    assert!(catch_unwind(AssertUnwindSafe(|| view.read(end, &mut [0]))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| view.read(end + 1, &mut []))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| view.read(u64::MAX, &mut [0]))).is_err());
    let mut frame = [0; 6];
    lease.read((START + 128) * PAGE, &mut frame);
    assert_eq!(&frame, b"suffix");
    lease.read(0, &mut frame[..5]);
    assert_eq!(&frame[..5], b"frame");
    drop(view);
    drop(lease);
    drop(runtime);
    let runtime = super::runtime(memory);
    let view = BTreeMapRegion::<u64, Vec<u8>, _>::new(
        Rc::new(
            runtime
                .open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
                .unwrap(),
        ),
        START..START + 128,
    );
    assert_eq!(view.get(&7).unwrap(), Some(vec![11]));
    assert_eq!(view.get(&8).unwrap(), Some(vec![12]));
}

#[test]
fn limited_snapshots_resume_and_survive_intervening_mutations() {
    use std::ops::Bound::{Excluded, Unbounded};
    let host = runtime(VectorMemory::default());
    let lease = Rc::new(
        host.open_memory(&crate::StableKey::parse("test.map.v1").unwrap())
            .unwrap(),
    );
    let mut view = BTreeMapRegion::<u64, u64>::new(lease, 0..END);
    // Zero must not try loading an uninitialized header.
    assert_eq!(view.keys_range_limited(.., 0).unwrap().len(), 0);
    view.initialize().unwrap();
    for key in 0..200 {
        view.insert(key, key + 1).unwrap();
    }
    let first = view.keys_range_limited(20..100, 3).unwrap();
    view.remove(&21).unwrap();
    view.insert(201, 202).unwrap();
    assert_eq!(first.collect::<Vec<_>>(), [20, 21, 22]);
    assert_eq!(
        view.keys_range_limited((Excluded(22), Unbounded), 3)
            .unwrap()
            .collect::<Vec<_>>(),
        [23, 24, 25]
    );
    assert_eq!(
        view.keys_range_limited(199.., 20)
            .unwrap()
            .collect::<Vec<_>>(),
        [199, 201]
    );
    assert_eq!(view.keys_range_limited(300.., 20).unwrap().len(), 0);
}
