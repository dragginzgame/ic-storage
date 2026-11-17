use super::*;
use crate::migration_job::{MapJob, MigrationJob, envelope_digest};
use std::cell::Cell;

thread_local! {
    static DECODES: Cell<usize> = const { Cell::new(0) };
}

struct CountedMaps;
impl sealed::Sealed for CountedMaps {}
impl Payload for CountedMaps {
    const MAGIC: &'static [u8; 8] = MapPair::MAGIC;
    const MAX_BYTES: usize = MapPair::MAX_BYTES;
    const DOMAINS: [u64; 2] = MapPair::DOMAINS;
    type Decoded = <MapPair as Payload>::Decoded;
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self::Decoded, Error> {
        DECODES.set(DECODES.get() + 1);
        MapPair::decode(bytes, epoch)
    }
    fn len(decoded: &Self::Decoded) -> usize {
        MapPair::len(decoded)
    }
}

#[test]
fn split_batch_decodes_once_per_request_and_never_bypasses_integrity() {
    type CountedJob = MigrationJob<CountedMaps>;
    let [a, b] = MapPair::DOMAINS;
    let maps = std::array::from_fn(|id| {
        let mut map = ic_storage_native::map::Map::new(64);
        map.with(|mut view| view.insert(id as u64, u64::MAX).unwrap())
            .unwrap();
        map
    });
    let mut job = MapJob::initial(a).unwrap().export(a, b, 1, &maps).unwrap();
    job.begin_import(b, a, 1).unwrap();
    let (metadata, payload) = job.encode_split().unwrap();
    DECODES.set(0);
    CountedJob::decode_split(&metadata, payload)
        .unwrap()
        .batch(b, a, 1, 0, 1)
        .unwrap();
    assert_eq!(DECODES.get(), 2);
    DECODES.set(0);
    let (advanced, batch) =
        CountedJob::decode_split_batch(&metadata, payload, b, a, 1, 0, 1).unwrap();
    assert_eq!(DECODES.get(), 1);
    assert_eq!((batch.end, batch.retry, batch.complete), (1, false, false));

    let (mut metadata, payload) = advanced.encode_split().unwrap();
    DECODES.set(0);
    let (_, retry) = CountedJob::decode_split_batch(&metadata, payload, b, a, 1, 0, 1).unwrap();
    assert_eq!(DECODES.get(), 1);
    assert!(retry.retry);

    let mut damaged = payload.to_vec();
    damaged[64] ^= 1;
    DECODES.set(0);
    assert_eq!(
        CountedJob::decode_split_batch(&metadata, &damaged, b, a, 1, 0, 1).err(),
        Some(Error::Integrity)
    );
    assert_eq!(DECODES.get(), 0);
    // Re-signing the outer job must not admit a corrupt nested checkpoint.
    let digest = envelope_digest(&metadata[..64], &damaged, &metadata[64..72]);
    metadata[72..].copy_from_slice(&digest);
    assert!(matches!(
        CountedJob::decode_split_batch(&metadata, &damaged, b, a, 1, 0, 1),
        Err(Error::MapCheckpoint(_))
    ));
    assert_eq!(DECODES.get(), 1);
}
