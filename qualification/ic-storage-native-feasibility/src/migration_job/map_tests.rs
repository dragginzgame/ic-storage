use super::*;
use ic_storage_native::map::Map;

const A: u64 = 131072;
const B: u64 = 163840;
fn maps() -> [Map; 2] {
    let mut maps = [Map::new(65), Map::new(64)];
    for (id, map) in maps.iter_mut().enumerate() {
        map.with(|mut view| {
            for key in 0..63 {
                view.insert(key, key + id as u64).unwrap();
            }
            view.insert(u64::MAX, u64::MAX - id as u64).unwrap();
        })
        .unwrap();
    }
    maps
}

#[test]
fn split_storage_preserves_canonical_bytes_and_binds_both_parts() {
    fn check(job: &MapJob) {
        let canonical = job.encode().unwrap();
        let (metadata, payload) = job.encode_split().unwrap();
        assert_eq!(metadata.len(), 104);
        assert_eq!(&metadata[..64], &canonical[..64]);
        assert_eq!(payload, &canonical[64..canonical.len() - 40]);
        assert_eq!(&metadata[64..], &canonical[canonical.len() - 40..]);
        assert_eq!(&MapJob::decode_split(&metadata, payload).unwrap(), job);
        for length in 0..metadata.len() {
            assert!(MapJob::decode_split(&metadata[..length], payload).is_err());
        }
        for index in 0..metadata.len() {
            let mut damaged = metadata;
            damaged[index] ^= 1;
            assert!(MapJob::decode_split(&damaged, payload).is_err());
        }
        if !payload.is_empty() {
            assert!(MapJob::decode_split(&metadata, &payload[..payload.len() - 1]).is_err());
            let mut damaged = payload.to_vec();
            damaged[64] ^= 1;
            assert!(MapJob::decode_split(&metadata, &damaged).is_err());
        }
        let mut extra = metadata.to_vec();
        extra.push(0);
        assert!(MapJob::decode_split(&extra, payload).is_err());
        assert!(
            MapJob::decode_split(&metadata, &vec![0; checkpoint::maps::MAX_BYTES + 1]).is_err()
        );
    }
    let ready = MapJob::initial(A).unwrap();
    check(&ready);
    let mut job = ready.export(A, B, 1, &maps()).unwrap();
    check(&job);
    job.cancel(A, B, 1).unwrap();
    check(&job);
    job = job.export(A, B, 2, &maps()).unwrap();
    job.begin_import(B, A, 2).unwrap();
    check(&job);
    job.batch(B, A, 2, 0, 64).unwrap();
    check(&job);
    job.batch(B, A, 2, 64, 64).unwrap();
    check(&job);
}

#[test]
fn map_jobs_bind_domains_epochs_and_both_payloads() {
    let mut source = maps();
    let ready = MapJob::initial(A).unwrap();
    assert_eq!(MapJob::decode(&ready.encode().unwrap()).unwrap(), ready);
    assert_eq!(MapJob::initial(147456), Err(Error::Domain));
    assert_eq!(Job::initial(B), Err(Error::Domain));
    assert_eq!(RecordJob::initial(B), Err(Error::Domain));
    assert_eq!(ready.export(A, B, 0, &source), Err(Error::Epoch));
    assert_eq!(ready.export(A, A, 1, &source), Err(Error::Domain));
    assert_eq!(ready.export(B, A, 1, &source), Err(Error::Domain));
    let sealed = ready.export(A, B, 1, &source).unwrap();
    assert_eq!(sealed.export(A, B, 1, &source).unwrap(), sealed);
    assert_eq!(sealed.export(A, B, 2, &source), Err(Error::Epoch));
    source[1]
        .with(|mut view| view.insert(1, 999).unwrap())
        .unwrap();
    assert_eq!(sealed.export(A, B, 1, &source), Err(Error::State));
    source[0]
        .with(|mut view| view.insert(999, 123).unwrap())
        .unwrap();
    assert_eq!(
        ready.export(A, B, 1, &source),
        Err(Error::MapCheckpoint(ic_storage_native::map::Error::Limit))
    );
    drop(source);
    let bytes = sealed.encode().unwrap();
    assert_eq!(bytes.len(), MAP_MAX_BYTES);
    assert!(Job::decode(&bytes).is_err());
    assert!(RecordJob::decode(&bytes).is_err());
    for offset in [0, 8, 10, 16, 24, 32, 44, 64, bytes.len() - 1] {
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 1;
        assert!(MapJob::decode(&corrupt).is_err());
    }
    let mut job = MapJob::decode(&bytes).unwrap();
    assert!(matches!(job.consume(A, B, 1), Err(Error::Domain)));
    assert!(matches!(job.consume(B, A, 2), Err(Error::Epoch)));
    assert_eq!(job, sealed);
    let mut recovered = job.consume(B, A, 1).unwrap();
    for (id, map) in recovered.iter_mut().enumerate() {
        assert_eq!(map.len().unwrap(), 64);
        assert_eq!(
            map.with(|view| view.get(u64::MAX)).unwrap(),
            Some(u64::MAX - id as u64)
        );
    }
    let consumed = MapJob::decode(&job.encode().unwrap()).unwrap();
    assert!(matches!(job.consume(B, A, 1), Err(Error::State)));
    assert_eq!(job.cancel(A, B, 1), Err(Error::State));
    assert_eq!(consumed.export(B, A, 1, &recovered), Err(Error::Epoch));
    assert!(consumed.export(B, A, 2, &recovered).is_ok());
    let mut cancelled = sealed;
    assert!(cancelled.cancel(A, B, 1).unwrap());
    let mut cancelled = MapJob::decode(&cancelled.encode().unwrap()).unwrap();
    assert!(!cancelled.cancel(A, B, 1).unwrap());
    assert!(matches!(cancelled.consume(B, A, 1), Err(Error::State)));
    assert!(cancelled.export(A, B, 2, &recovered).is_ok());
}

#[test]
fn map_progress_crosses_roots_retries_restarts_and_consumes_once() {
    let source = maps();
    let mut job = MapJob::initial(A)
        .unwrap()
        .export(A, B, 1, &source)
        .unwrap();
    job.begin_import(B, A, 1).unwrap();
    assert_eq!(job.cancel(A, B, 1), Err(Error::State));
    assert_eq!(job.export(A, B, 2, &source), Err(Error::State));
    for (start, limit, end) in [(0, 63, 63), (63, 2, 65)] {
        let batch = job.batch(B, A, 1, start, limit).unwrap();
        assert_eq!(
            (batch.start, batch.end, batch.complete, batch.retry),
            (start, end, false, false)
        );
        job = MapJob::decode(&job.encode().unwrap()).unwrap();
        let saved = job.encode().unwrap();
        assert!(job.batch(B, A, 1, start, limit).unwrap().retry);
        assert_eq!(job.encode().unwrap(), saved);
        assert!(matches!(
            job.batch(B, A, 1, start, limit + 1),
            Err(Error::State)
        ));
    }
    assert!(matches!(job.batch(B, A, 1, 0, 63), Err(Error::State)));
    job.restart_import(B, A, 1).unwrap();
    assert_eq!(job.importing(B, A, 1).unwrap().1, 0);
    job = MapJob::decode(&job.encode().unwrap()).unwrap();
    assert!(!job.batch(B, A, 1, 0, 64).unwrap().complete);
    assert!(job.batch(B, A, 1, 64, 64).unwrap().complete);
    let mut job = MapJob::decode(&job.encode().unwrap()).unwrap();
    assert!(job.batch(B, A, 1, 64, 64).unwrap().retry);
    assert!(matches!(job.restart_import(B, A, 1), Err(Error::State)));
    assert!(matches!(job.consume(B, A, 1), Err(Error::State)));
    assert!(job.export(B, A, 2, &source).is_ok());
}
