use super::*;
use alloc::collections::BTreeMap;

fn records(map: &mut Map) -> BTreeMap<u64, u64> {
    map.with(|view| {
        let mut values = BTreeMap::new();
        view.for_each(|key, value| {
            values.insert(key, value);
        });
        values
    })
    .unwrap()
}
fn seal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}
#[test]
fn two_maps_recover_independently_after_sources_are_destroyed() {
    for count in [0, 1, LIMIT] {
        let mut maps = [Map::new(LIMIT), Map::new(LIMIT)];
        for (id, map) in maps.iter_mut().enumerate() {
            map.with(|mut view| {
                for key in 0..count as u64 {
                    view.insert(u64::MAX - key, key ^ (u64::MAX - id as u64))
                        .unwrap();
                }
            })
            .unwrap();
        }
        let expected = [records(&mut maps[0]), records(&mut maps[1])];
        let bytes = encode(&maps, 17).unwrap();
        drop(maps);
        let mut restored = decode(&bytes, 17).unwrap();
        for (id, map) in restored.iter_mut().enumerate() {
            assert_eq!(records(map), expected[id]);
        }
        if count > 0 {
            restored[0]
                .with(|mut view| {
                    assert!(view.remove(u64::MAX).is_some());
                    view.insert(0, u64::MAX).unwrap();
                })
                .unwrap();
        }
        assert_eq!(records(&mut restored[1]), expected[1]);
    }
}
#[test]
fn framing_integrity_identity_and_inner_maps_fail_closed() {
    let mut maps = [Map::new(LIMIT + 1), Map::new(LIMIT)];
    for map in &mut maps {
        map.with(|mut view| {
            view.insert(11, 22).unwrap();
            view.insert(33, 44).unwrap();
        })
        .unwrap();
    }
    let good = encode(&maps, 17).unwrap();
    for length in 0..good.len() {
        assert!(decode(&good[..length], 17).is_err());
    }
    let mut extra = good.clone();
    extra.push(0);
    assert!(decode(&extra, 17).is_err());
    assert!(matches!(decode(&good, 18), Err(Error::Epoch)));
    for offset in [0, 8, 24, 32, 40, 48, 56] {
        let mut bad = good.clone();
        bad[offset] ^= 1;
        seal(&mut bad);
        assert!(decode(&bad, 17).is_err());
    }
    let mut bad = good.clone();
    bad[90] ^= 1;
    assert!(matches!(decode(&bad, 17), Err(Error::Integrity)));
    // Recomputed outer integrity cannot hide an inner digest/epoch violation.
    seal(&mut bad);
    assert!(matches!(decode(&bad, 17), Err(Error::Integrity)));
    let a = u64::from_le_bytes(good[40..48].try_into().unwrap()) as usize;
    let second = 64 + a;
    let mut bad = good.clone();
    bad[second + 8] ^= 1;
    seal(&mut bad[second..good.len() - 32]);
    seal(&mut bad);
    assert!(matches!(decode(&bad, 17), Err(Error::Epoch)));
    // Duplicate a logical key, repairing both digests: product admission refuses.
    let mut bad = good.clone();
    let key: [u8; 8] = bad[second + 24..second + 32].try_into().unwrap();
    bad[second + 40..second + 48].copy_from_slice(&key);
    seal(&mut bad[second..good.len() - 32]);
    seal(&mut bad);
    assert!(matches!(decode(&bad, 17), Err(Error::Checkpoint)));
    assert_eq!(records(&mut maps[0]), BTreeMap::from([(11, 22), (33, 44)]));
    maps[0]
        .with(|mut view| {
            for key in 0..=LIMIT as u64 {
                view.insert(key, key).unwrap();
            }
        })
        .unwrap();
    assert!(matches!(encode(&maps, 17), Err(Error::Limit)));
}
