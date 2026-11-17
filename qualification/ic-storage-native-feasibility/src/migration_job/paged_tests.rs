use super::*;
use checkpoint::maps::paged;
use ic_storage_native::map::Map;

const A: u64 = 131072;
const B: u64 = 163840;

#[test]
fn independent_consumed_empty_paged_job_golden() {
    // Perl pack/Digest::SHA from the documented fields, independent of Rust.
    let hex = include_str!("golden-paged-consumed.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let mut job = PagedMapJob::initial(A)
        .unwrap()
        .seal(A, B, 1, &manifest([0, 0], 1))
        .unwrap();
    job.begin_import(B, A, 1).unwrap();
    assert!(job.batch(B, A, 1, 0, 2).unwrap().complete);
    assert_eq!(job.encode().unwrap(), bytes);
    assert_eq!(PagedMapJob::decode(&bytes).unwrap(), job);
}

fn manifest(counts: [usize; 2], epoch: u64) -> paged::Manifest {
    let maps = counts.map(|count| {
        let mut map = Map::new(count);
        map.with(|mut view| {
            for key in 0..count as u64 {
                view.insert(key, u64::MAX - key).unwrap();
            }
        })
        .unwrap();
        map
    });
    paged::export(&maps, epoch, |_, _, _| Ok::<_, ()>(())).unwrap()
}

#[test]
fn paged_progress_reopens_retries_crosses_roots_and_retires_epochs() {
    for counts in [[0, 0], [0, 65], [65, 0], [129, 65], [2304, 2304]] {
        let pair = manifest(counts, 1);
        let ready = PagedMapJob::initial(A).unwrap();
        let mut job = ready.seal(A, B, 1, &pair).unwrap();
        assert_eq!(job.seal(A, B, 1, &pair).unwrap(), job);
        assert_eq!(job.frozen(A, B, 1).unwrap(), pair);
        assert_eq!(job.cancel(B, A, 1), Err(Error::Domain));
        assert!(job.cancel(A, B, 1).unwrap());
        assert!(!job.cancel(A, B, 1).unwrap());
        assert!(job.begin_import(B, A, 1).is_err());
        assert!(job.seal(A, B, 1, &pair).is_err());
        assert!(job.seal(A, B, 2, &manifest(counts, 2)).is_ok());
        job = ready.seal(A, B, 1, &pair).unwrap();
        assert_eq!(job.begin_import(B, A, 1).unwrap(), pair);
        assert!(job.cancel(A, B, 1).is_err());
        assert!(job.seal(A, B, 2, &manifest(counts, 2)).is_err());
        let mut start = 0;
        loop {
            let (metadata, payload) = job.encode_split().unwrap();
            assert_eq!(payload, pair.encode());
            assert_eq!(metadata.len(), 104);
            let (next, batch) =
                PagedMapJob::decode_split_batch(&metadata, payload, B, A, 1, start, 2).unwrap();
            assert!(!batch.retry);
            assert_eq!(batch.checkpoint, pair);
            assert_eq!(
                batch.end as usize,
                (start as usize + 2).min(pair.page_count())
            );
            job = PagedMapJob::decode(&next.encode().unwrap()).unwrap();
            let before = job.encode().unwrap();
            assert!(job.batch(B, A, 1, start, 2).unwrap().retry);
            assert_eq!(job.encode().unwrap(), before);
            assert!(job.batch(B, A, 1, start, 3).is_err());
            if batch.complete {
                break;
            }
            start = batch.end;
        }
        assert!(job.restart_import(B, A, 1).is_err());
        assert!(job.consume(B, A, 1).is_err());
        assert!(job.batch(B, A, 1, start + 2, 2).is_err());
        assert!(job.seal(B, A, 2, &manifest(counts, 2)).is_ok());
    }
}

#[test]
fn paged_binding_restart_and_corrupt_progress_refuse_without_mutation() {
    let pair = manifest([129, 65], 1);
    let ready = PagedMapJob::initial(A).unwrap();
    assert!(ready.seal(A, A, 1, &pair).is_err());
    assert!(ready.seal(B, A, 1, &pair).is_err());
    assert!(ready.seal(A, B, 2, &pair).is_err());
    assert!(ready.seal(A, B, 1, &manifest([129, 65], 2)).is_err());
    let mut job = ready.seal(A, B, 1, &pair).unwrap();
    assert!(job.seal(A, B, 1, &manifest([129, 64], 1)).is_err());
    job.begin_import(B, A, 1).unwrap();
    job.batch(B, A, 1, 0, 2).unwrap();
    let before = job.encode().unwrap();
    for (target, epoch, start, limit) in [
        (A, 1, 2, 1),
        (B, 2, 2, 1),
        (B, 1, 1, 1),
        (B, 1, 2, 0),
        (B, 1, 2, 65),
    ] {
        assert!(job.batch(target, A, epoch, start, limit).is_err());
        assert_eq!(job.encode().unwrap(), before);
    }
    assert_eq!(job.restart_import(B, A, 1).unwrap(), pair);
    job = PagedMapJob::decode(&job.encode().unwrap()).unwrap();
    assert_eq!(job.importing(B, A, 1).unwrap().1, 0);
    assert!(!job.batch(B, A, 1, 0, 2).unwrap().retry);
    let good = job.encode().unwrap();
    assert_eq!(good.len(), 104 + paged::MANIFEST_BYTES);
    assert!(MapJob::decode(&good).is_err());
    for length in 0..good.len() {
        assert!(PagedMapJob::decode(&good[..length]).is_err());
    }
    for index in 0..good.len() {
        let mut bad = good.clone();
        bad[index] ^= 1;
        assert!(PagedMapJob::decode(&bad).is_err());
    }
    for (offset, value) in [(44, 99u32), (48, 99), (52, 0)] {
        let (mut metadata, payload) = job.encode_split().unwrap();
        metadata[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        let digest = envelope_digest(&metadata[..64], payload, &metadata[64..72]);
        metadata[72..].copy_from_slice(&digest);
        assert!(PagedMapJob::decode_split(&metadata, payload).is_err());
    }
    let (metadata, _) = job.encode_split().unwrap();
    let foreign = manifest([129, 64], 1).encode();
    assert_eq!(
        PagedMapJob::decode_split(&metadata, &foreign),
        Err(Error::Integrity)
    );
}

#[test]
fn paged_job_rejects_page_offsets_wider_than_durable_progress() {
    let mut bytes = manifest([1, 0], 1).encode();
    // Canonical nested/outer checksums, but one more page than the job can count.
    let count = (u64::from(u32::MAX) + 1) * 64;
    bytes[64..72].copy_from_slice(&count.to_le_bytes());
    bytes[72..80].copy_from_slice(&(count / 64).to_le_bytes());
    let digest = Sha256::digest(&bytes[32..128]);
    bytes[128..160].copy_from_slice(&digest);
    let digest = Sha256::digest(&bytes[..296]);
    bytes[296..].copy_from_slice(&digest);
    assert!(PagedMapPair::decode(&bytes, 1).is_err());
}
