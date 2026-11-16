use super::*;
use crate::record::{V2, narrow::V3};
const A: u64 = 131072;
const B: u64 = 147456;

fn records(count: usize) -> Vec<V3> {
    (0..count)
        .map(|i| V3::new(i as u32, 100).unwrap())
        .collect()
}

fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn independent_record_identity_and_nested_validation() {
    let initial = RecordJob::initial(A).unwrap().encode().unwrap();
    let mut expected = b"ICJRV001".to_vec();
    expected.extend_from_slice(&[1, 0, 0, 0, 64, 0, 0, 0]);
    for word in [0u64, A, A] {
        expected.extend_from_slice(&word.to_le_bytes());
    }
    expected.extend_from_slice(&[0; 24]);
    expected.extend_from_slice(b"JOBDONE1");
    let digest = Sha256::digest(&expected);
    expected.extend_from_slice(&digest);
    assert_eq!(initial, expected);
    assert_eq!(Job::decode(&initial), Err(Error::Framing));
    assert_eq!(
        RecordJob::decode(&Job::initial(A).unwrap().encode().unwrap()),
        Err(Error::Framing)
    );
    let sealed = RecordJob::initial(A)
        .unwrap()
        .export(A, B, 1, &records(16))
        .unwrap();
    let bytes = sealed.encode().unwrap();
    assert_eq!(bytes.len(), RECORD_MAX_BYTES);
    assert_eq!(RecordJob::decode(&bytes).unwrap(), sealed);
    for end in 0..bytes.len() {
        assert!(RecordJob::decode(&bytes[..end]).is_err());
    }
    let mut invalid = bytes;
    // Reseal both layers with an invalid final ceiling: integrity is not validity.
    invalid[HEADER + 32 + 15 * 12 + 4..HEADER + 32 + 16 * 12].fill(0);
    let nested_end = invalid.len() - 40;
    reseal(&mut invalid[HEADER..nested_end]);
    reseal(&mut invalid);
    assert_eq!(
        RecordJob::decode(&invalid),
        Err(Error::RecordCheckpoint(
            checkpoint::record_vector::Error::Record(ic_storage_native::record::Error::Invariant)
        ))
    );
    let mut wrong = sealed;
    wrong.checkpoint = checkpoint::record_vector::encode_v2(&[V2::new(1, 2).unwrap()], 1).unwrap();
    assert!(matches!(
        RecordJob::decode(&wrong.encode().unwrap()),
        Err(Error::RecordCheckpoint(
            checkpoint::record_vector::Error::Schema
        ))
    ));
}

#[test]
fn record_jobs_share_epochs_cancellation_and_single_use() {
    let values = records(16);
    let initial = RecordJob::initial(A).unwrap();
    assert_eq!(initial.export(A, B, 0, &values), Err(Error::Epoch));
    assert!(matches!(
        initial.export(A, B, 1, &records(17)),
        Err(Error::RecordCheckpoint(
            checkpoint::record_vector::Error::Limit
        ))
    ));
    let mut job = initial.export(A, B, 1, &values).unwrap();
    assert_eq!(job.export(A, B, 1, &values).unwrap(), job);
    assert_eq!(job.export(A, B, 1, &records(15)), Err(Error::State));
    assert!(job.cancel(A, B, 1).unwrap());
    assert!(!job.cancel(A, B, 1).unwrap());
    assert!(job.consume(B, A, 1).is_err());
    let mut job = job.export(A, B, 2, &values).unwrap();
    let before = job.encode().unwrap();
    assert!(job.consume(A, B, 2).is_err());
    assert_eq!(job.encode().unwrap(), before);
    let decoded = job.consume(B, A, 2).unwrap();
    assert_eq!(decoded.1, 16);
    assert_eq!(decoded.0[15], Some(values[15]));
    assert!(job.consume(B, A, 2).is_err());
    assert!(job.cancel(A, B, 2).is_err());
    let mut last = job.export(B, A, 3, &values).unwrap();
    last.epoch = u64::MAX;
    last.checkpoint = checkpoint::record_vector::encode(&values, u64::MAX).unwrap();
    last.consume(A, B, u64::MAX).unwrap();
    assert_eq!(last.export(A, B, 0, &values), Err(Error::Epoch));
}

#[test]
fn record_progress_validates_retries_restart_and_completed_replay() {
    for count in [0, 1, 16] {
        let values = records(count);
        let mut job = RecordJob::initial(A)
            .unwrap()
            .export(A, B, 1, &values)
            .unwrap();
        job.begin_import(B, A, 1).unwrap();
        assert_eq!(job.export(B, A, 2, &values), Err(Error::State));
        let first = job.batch(B, A, 1, 0, 3).unwrap();
        assert_eq!(first.end, count.min(3) as u32);
        assert!(job.batch(B, A, 1, 0, 3).unwrap().retry);
        let saved = job.encode().unwrap();
        assert!(job.batch(B, A, 1, 0, 2).is_err());
        assert_eq!(job.encode().unwrap(), saved);
        job = RecordJob::decode(&saved).unwrap();
        if count > 3 {
            assert_eq!(job.restart_import(B, A, 1).unwrap().1, count);
            assert_eq!(job.importing(B, A, 1).unwrap().1, 0);
            job.batch(B, A, 1, 0, 64).unwrap();
        }
        assert!(job.restart_import(B, A, 1).is_err());
        assert!(job.begin_import(B, A, 1).is_err());
        assert!(job.consume(B, A, 1).is_err());
        assert!(RecordJob::decode(&job.encode().unwrap()).is_ok());
    }
}
