use super::*;
use alloc::vec;
use ic_storage::ordered::{Map, Mutation};

const A: u64 = OrderedRoot::DOMAINS[0];
const B: u64 = OrderedRoot::DOMAINS[1];

fn source() -> Map<512, 4096> {
    let mut map = Map::new(128);
    for id in 0..64u64 {
        let key = id.to_be_bytes();
        let index = (id + 64).to_be_bytes();
        map.with(|mut v| {
            v.apply(&[
                Mutation::Put(&key, &vec![id as u8; 4096]),
                Mutation::Put(&index, &key),
            ])
        })
        .unwrap()
        .unwrap();
    }
    map
}
fn export(map: &Map<512, 4096>, epoch: u64) -> (Manifest, Vec<Vec<u8>>) {
    let mut pages = Vec::new();
    let manifest = map
        .export_pages(epoch, 1, |_, bytes| {
            pages.push(bytes.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    pages.reverse();
    (manifest, pages)
}

#[test]
fn ordered_jobs_reopen_between_batches_and_preserve_completed_writes() {
    let mut source = source();
    let mut job = OrderedJob::initial(A).unwrap();
    for (epoch, origin, target) in [(1, A, B), (2, B, A)] {
        let expected = source.checkpoint(epoch).unwrap();
        let (manifest, pages) = export(&source, epoch);
        job = job.seal(origin, target, epoch, &manifest).unwrap();
        assert!(
            job.reader(&manifest.reader::<512, 4096>().checkpoint().unwrap())
                .is_err()
        );
        job.begin_import(target, origin, epoch).unwrap();
        drop(source);
        let mut candidate = Map::<512, 4096>::new(128);
        let mut cursor = manifest.reader::<512, 4096>().checkpoint().unwrap();
        for (index, page) in pages.into_iter().enumerate() {
            // Reopen the byte-defined job and private native prefix between steps.
            job = OrderedJob::decode(&job.encode().unwrap()).unwrap();
            candidate =
                Map::admit(candidate.release().unwrap(), 128).unwrap_or_else(|_| panic!("prefix"));
            let committed = job.encode().unwrap();
            let mut next_job = OrderedJob::decode(&committed).unwrap();
            let batch = next_job
                .batch(target, origin, epoch, index as u32, 1)
                .unwrap();
            assert!(!batch.retry);
            let mut reader = job.reader(&cursor).unwrap();
            assert_eq!(reader.next_page(), Some(index));
            let records = reader.read_page(&page).unwrap();
            let mutations: Vec<_> = records
                .records()
                .map(|(k, v)| Mutation::Put(k, v))
                .collect();
            let next_cursor = reader.checkpoint().unwrap();
            assert!(job.reader(&next_cursor).is_err());
            next_job.reader(&next_cursor).unwrap();
            // Only this coherent triple may be committed by a real transaction.
            candidate
                .with(|mut v| v.apply(&mutations))
                .unwrap()
                .unwrap();
            job = next_job;
            cursor = next_cursor;
            let before_retry = job.encode().unwrap();
            assert!(
                job.batch(target, origin, epoch, index as u32, 1)
                    .unwrap()
                    .retry
            );
            assert_eq!(job.encode().unwrap(), before_retry);
            assert!(job.batch(target, origin, epoch, index as u32, 2).is_err());
        }
        assert_eq!(job.reader(&cursor).unwrap().next_page(), None);
        assert_eq!(candidate.checkpoint(epoch).unwrap(), expected);
        candidate
            .with(|mut v| v.insert(&0u64.to_be_bytes(), b"new application write"))
            .unwrap()
            .unwrap();
        let after = candidate.checkpoint(epoch).unwrap();
        let last = manifest.page_count() as u32 - 1;
        assert!(job.batch(target, origin, epoch, last, 1).unwrap().retry);
        assert_eq!(candidate.checkpoint(epoch).unwrap(), after);
        assert!(job.restart_import(target, origin, epoch).is_err());
        assert!(job.begin_import(target, origin, epoch).is_err());
        source = candidate;
    }
}

#[test]
fn ordered_job_refusals_cancellation_and_explicit_restart() {
    let (manifest, pages) = export(&source(), 1);
    let ready = OrderedJob::initial(A).unwrap();
    assert!(ready.seal(A, A, 1, &manifest).is_err());
    assert!(ready.seal(A, B, 2, &manifest).is_err());
    let mut cancelled = ready.seal(A, B, 1, &manifest).unwrap();
    assert!(cancelled.cancel(A, B, 1).unwrap());
    assert!(!cancelled.cancel(A, B, 1).unwrap());
    assert!(cancelled.begin_import(B, A, 1).is_err());
    let mut job = ready.seal(A, B, 1, &manifest).unwrap();
    job.begin_import(B, A, 1).unwrap();
    let mut cursor = manifest.reader::<512, 4096>().checkpoint().unwrap();
    let committed = job.encode().unwrap();
    assert!(job.batch(A, B, 1, 0, 1).is_err());
    assert!(job.batch(B, A, 2, 0, 1).is_err());
    assert!(job.batch(B, A, 1, 1, 1).is_err());
    assert_eq!(job.encode().unwrap(), committed);
    let mut next_job = OrderedJob::decode(&committed).unwrap();
    next_job.batch(B, A, 1, 0, 1).unwrap();
    let mut reader = job.reader(&cursor).unwrap();
    let mut corrupt = pages[0].clone();
    corrupt[100] ^= 1;
    assert!(reader.read_page(&corrupt).is_err());
    assert_eq!(reader.checkpoint().unwrap(), cursor);
    let records = reader.read_page(&pages[0]).unwrap();
    let mutations: Vec<_> = records
        .records()
        .map(|(k, v)| Mutation::Put(k, v))
        .collect();
    let mut exhausted = Map::<512, 4096>::new(0);
    assert_eq!(
        exhausted.with(|mut v| v.apply(&mutations)).unwrap(),
        Err(ic_storage::ordered::Error::Limit)
    );
    // Neither staged job nor reader replaces the committed bytes after refusal.
    assert_eq!(job.encode().unwrap(), committed);
    job.reader(&cursor).unwrap();
    assert!(next_job.reader(&cursor).is_err());
    let mut candidate = Map::<512, 4096>::new(128);
    candidate
        .with(|mut view| view.apply(&mutations))
        .unwrap()
        .unwrap();
    cursor = reader.checkpoint().unwrap();
    job = next_job;
    job.reader(&cursor).unwrap();
    assert!(job.cancel(A, B, 1).is_err());
    drop(candidate); // Explicit restart discards the unfinished native prefix.
    job.restart_import(B, A, 1).unwrap();
    assert!(job.reader(&cursor).is_err());
    cursor = manifest.reader::<512, 4096>().checkpoint().unwrap();
    assert_eq!(job.reader(&cursor).unwrap().next_page(), Some(0));
}

#[test]
fn empty_ordered_job_completes_once_without_pages() {
    let (manifest, pages) = export(&Map::new(128), 1);
    assert!(pages.is_empty());
    let mut job = OrderedJob::initial(A)
        .unwrap()
        .seal(A, B, 1, &manifest)
        .unwrap();
    job.begin_import(B, A, 1).unwrap();
    let cursor = manifest.reader::<512, 4096>().checkpoint().unwrap();
    assert_eq!(job.reader(&cursor).unwrap().next_page(), None);
    let batch = job.batch(B, A, 1, 0, 1).unwrap();
    assert!(batch.complete && !batch.retry);
    let mut job = OrderedJob::decode(&job.encode().unwrap()).unwrap();
    job.reader(&cursor).unwrap();
    assert!(job.batch(B, A, 1, 0, 1).unwrap().retry);
}
