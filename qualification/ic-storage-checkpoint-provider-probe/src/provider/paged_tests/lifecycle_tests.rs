//! Host transition admission and governed writes; installed tests own rollback.
use super::super::paged::Jobs;
use super::*;
use ic_storage_native_feasibility::migration_job::paged_continuation::NativePrefix;

const A: u64 = 131072;
const B: u64 = 393216;

#[test]
fn cancelled_epochs_replace_page_counts_and_reject_stale_or_unknown_state() {
    let backing = TestMemory::default();
    let mut store = Jobs::open(backing.clone(), true);
    assert!(catch_unwind(AssertUnwindSafe(|| store.read())).is_err());
    store.initialize(A);
    for (offset, counts) in [[129, 65], [1, 0], [0, 0], [192, 192]]
        .into_iter()
        .enumerate()
    {
        let epoch = offset as u64 + 1;
        let source = create_sources(counts);
        store.publish_epoch(&source, A, B, epoch, |_| {});
        let manifest = store.read().frozen(A, B, epoch).unwrap();
        store.admit_pages(&manifest, 192);
        assert_eq!(
            store.data.pages.len(),
            counts.iter().map(|n| n.div_ceil(64)).sum::<usize>() as u64
        );
        // Every replaced page must carry this epoch and the correct records.
        for (root, expected) in counts.into_iter().enumerate() {
            let mut reader = manifest.roots()[root].reader();
            let mut records = Vec::new();
            while let Some(index) = reader.next_page() {
                records.extend(
                    reader
                        .read_page(&store.page(&manifest, root, index))
                        .unwrap(),
                );
            }
            assert_eq!(records.len(), expected);
            for (key, value) in records {
                assert_eq!(value, u64::MAX - key + root as u64);
            }
        }
        backing.reset();
        for bad in [(A, B, epoch + 1), (B, A, epoch), (A, B, epoch - 1)] {
            assert!(catch_unwind(AssertUnwindSafe(|| store.cancel(bad.0, bad.1, bad.2))).is_err());
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| store.publish_epoch(
                &source,
                A,
                B,
                epoch,
                |_| {}
            )))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
        assert!(store.cancel(A, B, epoch));
        assert_eq!(backing.counts().write_bytes, 104);
        backing.reset();
        assert!(!store.cancel(A, B, epoch));
        assert_eq!(backing.counts().writes, 0);
        drop(store);
        store = Jobs::open(backing.clone(), false);
    }
    // An empty or corrupt envelope is never a reset instruction.
    store.metadata.set([0; 104]).unwrap();
    backing.reset();
    assert!(
        catch_unwind(AssertUnwindSafe(|| store.publish_epoch(
            &create_sources([0, 0]),
            A,
            B,
            1,
            |_| {}
        )))
        .is_err()
    );
    assert_eq!(backing.counts().writes, 0);
}

#[test]
fn next_epoch_requires_the_completed_continuation_but_preserves_newer_values() {
    let backing = TestMemory::default();
    let mut store = Jobs::open(backing.clone(), true);
    store.initialize(A);
    let mut source = create_sources([1, 1]);
    store.publish_epoch(&source, A, B, 1, |_| {});
    let mut job = store.read();
    let manifest = job.begin_import(B, A, 1).unwrap();
    let mut readers = manifest.roots().each_ref().map(|root| root.reader());
    let prefixes = NativePrefix::from_maps(&create_sources([0, 0])).unwrap();
    store.commit_progress(&job, &readers, &prefixes, |_| {});
    backing.reset();
    assert!(
        catch_unwind(AssertUnwindSafe(|| store.publish_epoch(
            &source,
            B,
            A,
            2,
            |_| {}
        )))
        .is_err()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| store.cancel(A, B, 1))).is_err());
    assert_eq!(backing.counts().writes, 0);
    let batch = job.batch(B, A, 1, 0, 2).unwrap();
    assert!(batch.complete);
    for (root, reader) in readers.iter_mut().enumerate() {
        reader.read_page(&store.page(&manifest, root, 0)).unwrap();
    }
    let prefixes = NativePrefix::from_maps(&source).unwrap();
    store.commit_progress(&job, &readers, &prefixes, |_| {});
    source[0]
        .with(|mut view| view.insert(u64::MAX, 777).unwrap())
        .unwrap();
    let saved = *store.continuation.get();
    for bad in [[0; 512], {
        let mut bytes = saved;
        bytes[120] ^= 1;
        bytes
    }] {
        store.continuation.set(bad).unwrap();
        backing.reset();
        assert!(
            catch_unwind(AssertUnwindSafe(|| store.publish_epoch(
                &source,
                B,
                A,
                2,
                |_| {}
            )))
            .is_err()
        );
        assert_eq!(backing.counts().writes, 0);
    }
    store.continuation.set(saved).unwrap();
    store.publish_epoch(&source, B, A, 2, |_| {});
    assert_eq!(store.continuation.get(), &[0; 512]);
    let manifest = store.read().frozen(B, A, 2).unwrap();
    let records = manifest.roots()[0]
        .reader()
        .read_page(&store.page(&manifest, 0, 0))
        .unwrap();
    assert_eq!(records, [(u64::MAX, 777)]);
}
