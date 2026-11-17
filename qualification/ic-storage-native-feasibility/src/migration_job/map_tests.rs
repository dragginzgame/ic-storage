use super::*;
use ic_storage_native::map::Map;

const A: u64 = 131072;
const B: u64 = 163840;

#[test]
fn combined_batch_matches_separate_admission_for_states_and_requests() {
    fn compare(job: &MapJob, start: u32, limit: u32, target: u64, epoch: u64) {
        let (metadata, payload) = job.encode_split().unwrap();
        let mut separate = MapJob::decode_split(&metadata, payload).unwrap();
        let expected = separate.batch(target, A, epoch, start, limit);
        let combined =
            MapJob::decode_split_batch(&metadata, payload, target, A, epoch, start, limit);
        match (expected, combined) {
            (Ok(expected), Ok((actual_job, actual))) => {
                assert_eq!(actual_job, separate);
                assert_eq!(
                    (actual.start, actual.end, actual.retry, actual.complete),
                    (
                        expected.start,
                        expected.end,
                        expected.retry,
                        expected.complete
                    )
                );
                assert_eq!(actual.checkpoint, expected.checkpoint);
            }
            (Err(expected), Err(actual)) => assert_eq!(actual, expected),
            _ => panic!("combined admission changed the batch outcome"),
        }
    }
    let ready = MapJob::initial(A).unwrap();
    compare(&ready, 0, 1, A, 0);
    for counts in [[0, 0], [0, 64], [64, 0], [1, 64], [64, 64]] {
        let maps = std::array::from_fn(|id| {
            let mut map = Map::new(64);
            map.with(|mut view| {
                for key in 0..counts[id] {
                    view.insert(key, u64::MAX - key).unwrap();
                }
            })
            .unwrap();
            map
        });
        let mut job = ready.export(A, B, 1, &maps).unwrap();
        compare(&job, 0, 1, B, 1);
        job.cancel(A, B, 1).unwrap();
        compare(&job, 0, 1, B, 1);
        job = ready.export(A, B, 1, &maps).unwrap();
        job.begin_import(B, A, 1).unwrap();
        let mut next = 0;
        loop {
            for start in [0, 1, 63, 64, 65, 126, 128, u32::MAX] {
                for limit in [0, 1, 2, 63, 64, 65, u32::MAX] {
                    compare(&job, start, limit, B, 1);
                }
            }
            compare(&job, next, 63, A, 1);
            compare(&job, next, 63, B, 2);
            let (metadata, payload) = job.encode_split().unwrap();
            let (advanced, batch) =
                MapJob::decode_split_batch(&metadata, payload, B, A, 1, next, 63).unwrap();
            job = advanced;
            compare(&job, next, 63, B, 1); // latest retry, including completion
            if batch.complete {
                compare(&job, batch.end, 1, B, 1);
                break;
            }
            next = batch.end;
        }
    }
}

#[test]
fn independent_staged_map_envelope_golden() {
    // Independently assembled with Perl pack/Digest::SHA from the documented
    // three nested envelopes: one record per map, first record already imported.
    let hex = include_str!("golden-map-staged.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let mut maps = [Map::new(64), Map::new(64)];
    maps[0]
        .with(|mut view| view.insert(u64::MAX, u64::MAX - 1).unwrap())
        .unwrap();
    maps[1]
        .with(|mut view| view.insert(7, 70).unwrap())
        .unwrap();
    let mut job = MapJob::initial(A).unwrap().export(A, B, 1, &maps).unwrap();
    job.begin_import(B, A, 1).unwrap();
    job.batch(B, A, 1, 0, 1).unwrap();
    assert_eq!(bytes.len(), 344);
    assert_eq!(job.encode().unwrap(), bytes);
    assert_eq!(MapJob::decode(&bytes).unwrap(), job);
    let (metadata, checkpoint) = job.encode_split().unwrap();
    assert_eq!(&metadata[..64], &bytes[..64]);
    assert_eq!(checkpoint, &bytes[64..304]);
    assert_eq!(&metadata[64..], &bytes[304..]);
    assert_eq!(MapJob::decode_split(&metadata, checkpoint).unwrap(), job);
}

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
    let recovered = job.consume(B, A, 1).unwrap();
    for (id, records) in recovered.iter().enumerate() {
        assert_eq!(records.len(), 64);
        assert_eq!(records.last(), Some(&(u64::MAX, u64::MAX - id as u64)));
    }
    // The adapter constructs new native owners only after logical admission.
    let recovered = recovered.map(|records| {
        let mut map = Map::new(64);
        map.with(|mut view| {
            for (key, value) in records {
                assert!(view.insert(key, value).unwrap().is_none());
            }
        })
        .unwrap();
        map
    });
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
