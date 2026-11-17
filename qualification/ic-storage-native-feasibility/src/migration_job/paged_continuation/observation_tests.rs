use super::{
    tests::{fixture, step},
    *,
};
use alloc::vec::Vec;

fn observe(maps: &mut [Map; 2]) -> [NativePrefix; 2] {
    maps.each_mut().map(|map| {
        let mut records = Vec::new();
        map.with(|view| view.for_each(|key, value| records.push((key, value))))
            .unwrap();
        NativePrefix::from_records(records.len(), records).unwrap()
    })
}

#[test]
fn scalar_observations_preserve_map_bytes_across_every_page() {
    for counts in [[0, 0], [0, 65], [65, 0], [129, 65], [2304, 2304]] {
        let mut f = fixture(counts);
        for start in 0.. {
            let prefixes = observe(&mut f.maps);
            assert_eq!(prefixes, NativePrefix::from_maps(&f.maps).unwrap());
            let bytes = encode_observed(&f.job, &f.readers, &prefixes).unwrap();
            assert_eq!(bytes, encode(&f.job, &f.readers, &f.maps).unwrap());
            assert!(decode_observed(&bytes, &f.job, || Ok(prefixes)).is_ok());
            if f.job.phase == Phase::Consumed {
                break;
            }
            step(&mut f.job, &mut f.readers, &mut f.maps, &f.pages, start);
        }
    }
}

#[test]
fn observations_bind_count_values_and_bucket_order() {
    let records = [(u64::MAX, 2), (0, u64::MAX)];
    let prefix = NativePrefix::from_records(2, records).unwrap();
    assert_eq!(prefix.len(), 2);
    assert!(!prefix.is_empty());
    assert_ne!(
        prefix,
        NativePrefix::from_records(2, records.into_iter().rev()).unwrap()
    );
    assert!(NativePrefix::from_records(1, records).is_err());
    assert!(NativePrefix::from_records(3, records).is_err());
    let mut f = fixture([129, 65]);
    step(&mut f.job, &mut f.readers, &mut f.maps, &f.pages, 0);
    let bytes = encode(&f.job, &f.readers, &f.maps).unwrap();
    let empty = NativePrefix::from_records(0, []).unwrap();
    assert!(empty.is_empty());
    assert!(encode_observed(&f.job, &f.readers, &[empty.clone(), empty.clone()]).is_err());
    assert!(decode_observed(&bytes, &f.job, || Ok([empty.clone(), empty])).is_err());
    let mut records = Vec::new();
    f.maps[0]
        .with(|view| view.for_each(|k, v| records.push((k, v))))
        .unwrap();
    records[0].1 ^= 1;
    let mut prefixes = observe(&mut f.maps);
    prefixes[0] = NativePrefix::from_records(records.len(), records).unwrap();
    assert_eq!(
        decode_observed(&bytes, &f.job, || Ok(prefixes)).err(),
        Some(Error::Integrity)
    );
}

#[test]
fn invalid_binding_and_consumed_retry_never_observe_native_state() {
    let mut f = fixture([129, 65]);
    let old = encode(&f.job, &f.readers, &f.maps).unwrap();
    for length in 0..old.len() {
        assert!(
            decode_observed(&old[..length], &f.job, || panic!(
                "invalid framing scanned native state"
            ))
            .is_err()
        );
    }
    for index in 0..old.len() {
        let mut bad = old;
        bad[index] ^= 1;
        assert!(
            decode_observed(&bad, &f.job, || panic!(
                "invalid digest scanned native state"
            ))
            .is_err()
        );
    }
    assert_eq!(
        decode_observed(&old, &f.job, || Err(Error::State)).err(),
        Some(Error::State)
    );
    let mut calls = 0;
    assert!(
        decode_observed(&old, &f.job, || {
            calls += 1;
            Ok(observe(&mut f.maps))
        })
        .is_ok()
    );
    assert_eq!(calls, 1);
    step(&mut f.job, &mut f.readers, &mut f.maps, &f.pages, 0);
    assert!(decode_observed(&old, &f.job, || panic!("stale job scanned native state")).is_err());
    let mut mixed = encode(&f.job, &f.readers, &f.maps).unwrap();
    mixed[CURSORS..PREFIXES].copy_from_slice(&old[CURSORS..PREFIXES]);
    let digest = Sha256::digest(&mixed[..SEAL + 8]);
    mixed[SEAL + 8..].copy_from_slice(&digest);
    assert!(
        decode_observed(&mixed, &f.job, || panic!(
            "invalid cursors scanned native state"
        ))
        .is_err()
    );
    for start in 1.. {
        if step(&mut f.job, &mut f.readers, &mut f.maps, &f.pages, start) {
            break;
        }
    }
    let bytes = encode(&f.job, &f.readers, &f.maps).unwrap();
    assert!(
        decode_observed(&bytes, &f.job, || panic!(
            "consumed retry scanned active roots"
        ))
        .is_ok()
    );
}
