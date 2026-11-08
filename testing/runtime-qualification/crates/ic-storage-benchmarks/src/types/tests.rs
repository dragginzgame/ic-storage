use super::*;
fn check_keys<K: BenchKey + std::fmt::Debug>() {
    let ids = [0, 1, 9, 10, 255, 256, 16_415, u64::MAX];
    let keys: Vec<_> = ids.into_iter().map(K::from_id).collect();
    assert!(keys.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(keys.iter().map(K::id).collect::<Vec<_>>(), ids);
}
#[test]
fn concrete_keys_preserve_order_and_identity() {
    check_keys::<u64>();
    check_keys::<u128>();
    check_keys::<Vec<u8>>();
    check_keys::<BoundedBytes<255>>();
    check_keys::<TupleKey>();
    check_keys::<[u8; 64]>();
    let ids = [0, 1, 31, 32, 33, 16_415, i64::MAX as u64];
    let keys = ids.map(Signed64::from_id);
    assert!(keys.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(keys.map(|k| k.id()), ids);
    assert_eq!(keys[2..5], [-1, 0, 1].map(Signed64));
    check_keys::<[u8; 32]>();
    check_keys::<String>();
    let narrow = [0, 1, 255, 256, 65_535, 65_536, u64::from(u32::MAX)];
    let keys = narrow.map(u32::from_id);
    assert!(keys.windows(2).all(|p| p[0] < p[1]));
    assert_eq!(keys.map(|key| key.id()), narrow);
}
#[test]
fn values_cover_declared_sizes_and_record_fields() {
    for &case in DEFAULT_CASES {
        for &payload in case.payloads() {
            for key in [0, 255, 16_415, u64::MAX] {
                let before = case.expected_value(key, payload, false);
                let after = case.expected_value(key, payload, true);
                assert_eq!(before.len(), payload as usize);
                assert_eq!(after.len(), payload as usize);
                if payload > 0 {
                    assert_ne!(before, after);
                }
                if case == crate::case!(Map, String, String) && payload > 0 {
                    assert!(
                        std::str::from_utf8(&before)
                            .unwrap()
                            .contains(|c: char| !c.is_ascii())
                    );
                }
            }
        }
    }
    for replacement in [false, true] {
        let record = WireRecord::make(u64::MAX, 60, replacement);
        let encoded = record.encode().unwrap();
        assert!(encoded.len() <= 256);
        assert_eq!(WireRecord::decode(&encoded).unwrap(), record);
        assert_eq!(&record.clone().canonical()[..8], &u64::MAX.to_le_bytes());
        assert_eq!(
            &record.canonical()[24..28],
            &u32::from(replacement).to_le_bytes()
        );
    }
}
