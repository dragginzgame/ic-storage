use super::*;
use crate::provider::memory::TestMemory;
use ic_storage::map::Map;
use std::panic::{AssertUnwindSafe, catch_unwind};

const A: u64 = 131072;
const B: u64 = 163840;

fn maps(count: u64) -> [Map; 2] {
    std::array::from_fn(|id| {
        let mut map = Map::new(64);
        map.with(|mut view| {
            for key in 0..count {
                view.insert(key, key + id as u64).unwrap();
            }
        })
        .unwrap();
        map
    })
}

#[test]
fn progress_writes_are_fixed_size_and_retries_write_nothing() {
    for count in [0, 1, 64] {
        let backing = TestMemory::default();
        let mut store = Store::open(backing.clone(), true);
        let ready = MapJob::initial(A).unwrap();
        store.write(&ready);
        let mut job = ready.export(A, B, 1, &maps(count)).unwrap();
        backing.reset();
        store.write(&job);
        let payload = store.checkpoint.get().clone();
        assert_eq!(
            backing.counts().write_bytes,
            // Only the checkpoint length changes; metadata was already initialized.
            (payload.len() + METADATA_BYTES + 8) as u64
        );
        assert!(!backing.counts().overflowed);
        drop(store);
        backing.reset();
        let mut store = Store::open(backing.clone(), false);
        assert_eq!(store.read(), job);
        assert_eq!(backing.counts().writes, 0);

        job.begin_import(B, A, 1).unwrap();
        backing.reset();
        store.write(&job);
        assert_eq!(backing.counts().write_bytes, METADATA_BYTES as u64);
        let mut next = 0;
        loop {
            let (advanced, batch) = store.batch(B, A, 1, next, 63);
            job = advanced;
            backing.reset();
            store.write(&job);
            let io = backing.counts();
            assert_eq!(
                (io.writes, io.write_bytes, io.grows, io.overflowed),
                (1, 104, 0, false)
            );
            assert_eq!(store.checkpoint.get(), &payload);
            assert_eq!(store.read(), job);
            let (retried, retry) = store.batch(B, A, 1, next, 63);
            assert!(retry.retry);
            assert_eq!(retried, job);
            backing.reset();
            store.write(&retried);
            assert_eq!(backing.counts().writes, 0);
            if batch.complete {
                break;
            }
            next = batch.end;
        }
        println!(
            "map job records={} progress_write_bytes=104 retry_write_bytes=0",
            2 * count
        );
    }
}

#[test]
fn cancel_restart_and_atomic_consumption_preserve_payload() {
    let backing = TestMemory::default();
    let mut store = Store::open(backing.clone(), true);
    let ready = MapJob::initial(A).unwrap();
    store.write(&ready);
    let mut job = ready.export(A, B, 1, &maps(64)).unwrap();
    store.write(&job);
    let payload = store.checkpoint.get().clone();
    job.cancel(A, B, 1).unwrap();
    backing.reset();
    store.write(&job);
    assert_eq!(backing.counts().write_bytes, 104);
    job = job.export(A, B, 2, &maps(64)).unwrap();
    store.write(&job);
    assert_ne!(store.checkpoint.get(), &payload); // the checkpoint binds epoch 2
    let payload = store.checkpoint.get().clone();
    job.begin_import(B, A, 2).unwrap();
    job.batch(B, A, 2, 0, 64).unwrap();
    store.write(&job);
    job.restart_import(B, A, 2).unwrap();
    backing.reset();
    store.write(&job);
    assert_eq!(backing.counts().write_bytes, 104);
    assert_eq!(store.checkpoint.get(), &payload);
    let mut atomic = ready.export(A, B, 1, &maps(1)).unwrap();
    store.write(&atomic);
    atomic.consume(B, A, 1).unwrap();
    backing.reset();
    store.write(&atomic);
    assert_eq!(backing.counts().write_bytes, 104);
    assert_eq!(store.read(), atomic);
}

#[test]
fn missing_parts_and_old_single_cell_locators_refuse_without_creating_state() {
    for keys in [vec![], vec![METADATA], vec![CHECKPOINT], vec![99]] {
        let backing = TestMemory::default();
        if !keys.is_empty() {
            let mut storage = super::super::backing(backing.clone(), &keys, true);
            for key in keys {
                let region = storage.take(key).unwrap();
                drop(Record::create(region, Vec::<u8>::new()).unwrap());
            }
        }
        backing.reset();
        assert!(catch_unwind(AssertUnwindSafe(|| Store::open(backing.clone(), false))).is_err());
        assert_eq!(backing.counts().writes, 0);
    }
}

#[test]
fn corrupt_truncated_and_mixed_parts_refuse_without_writes() {
    for mode in 0..7 {
        let backing = TestMemory::default();
        let mut store = Store::open(backing.clone(), true);
        let ready = MapJob::initial(A).unwrap();
        let mut job = ready.export(A, B, 1, &maps(64)).unwrap();
        job.begin_import(B, A, 1).unwrap();
        store.write(&job);
        match mode {
            0 | 1 => store.damage(mode),
            2 => {
                let mut bytes = store.metadata.get().clone();
                bytes.pop();
                store.metadata.set(bytes).unwrap();
            }
            3 => {
                let mut bytes = store.checkpoint.get().clone();
                bytes.pop();
                store.checkpoint.set(bytes).unwrap();
            }
            4 => {
                let mut changed = maps(64);
                changed[1]
                    .with(|mut view| view.insert(0, 999).unwrap())
                    .unwrap();
                let other = ready.export(A, B, 1, &changed).unwrap();
                store
                    .checkpoint
                    .set(other.encode_split().unwrap().1.to_vec())
                    .unwrap();
            }
            5 => {
                store.metadata.set(vec![0; METADATA_BYTES + 1]).unwrap();
            }
            6 => {
                store.checkpoint.set(vec![0; maps::MAX_BYTES + 1]).unwrap();
            }
            _ => unreachable!(),
        }
        drop(store);
        backing.reset();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || Store::open(backing.clone(), false).read()
            ))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                Store::open(backing.clone(), false).batch(B, A, 1, 0, 64)
            }))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
    }
}
