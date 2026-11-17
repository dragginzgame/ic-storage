use super::*;

fn expected(count: usize) -> Vec<(u64, u64)> {
    (0..count)
        .map(|index| {
            let key = u64::MAX - (count - index - 1) as u64;
            (key, key ^ 0xa55a_0123_4567_89ab)
        })
        .collect()
}

#[test]
fn scalar_prefix_verification_covers_every_length_and_record_position() {
    for count in 0..=LIMIT {
        let records = expected(count);
        // Every checkpoint position can appear first in native bucket order.
        for rotation in 0..count.max(1) {
            let mut calls = 0;
            assert_eq!(
                verify_records(&records, count, |index| {
                    assert_eq!(index, calls);
                    calls += 1;
                    records[(index + rotation) % count]
                }),
                Ok(())
            );
            assert_eq!(calls, count);
        }
        assert_eq!(
            verify_records(&records, count, |index| records[count - index - 1]),
            Ok(())
        );
    }
}

#[test]
fn mismatched_counts_and_invalid_expected_records_refuse_before_reading() {
    let records = expected(LIMIT);
    for count in [0, LIMIT - 1, LIMIT + 1, usize::MAX] {
        assert!(verify_records(&records, count, |_| panic!("must not read")).is_err());
    }
    let too_many = expected(LIMIT + 1);
    assert_eq!(
        verify_records(&too_many, too_many.len(), |_| panic!("must not read")),
        Err(Error::Limit)
    );
    for records in [alloc::vec![(2, 4), (1, 3)], alloc::vec![(1, 3), (1, 3)]] {
        assert_eq!(
            verify_records(&records, records.len(), |_| panic!("must not read")),
            Err(Error::Checkpoint)
        );
    }
}

#[test]
fn equal_counts_cannot_hide_duplicate_missing_or_corrupt_records() {
    let records = expected(LIMIT);
    for index in 0..LIMIT {
        for replacement in [(0, 1), (records[index].0, records[index].1 ^ 1)] {
            assert_eq!(
                verify_records(&records, LIMIT, |at| {
                    if at == index {
                        replacement
                    } else {
                        records[at]
                    }
                }),
                Err(Error::Checkpoint)
            );
        }
        // All duplicate placements, including the high bit of the full bitmap.
        for duplicate in 0..LIMIT {
            if duplicate == index {
                continue;
            }
            assert_eq!(
                verify_records(&records, LIMIT, |at| {
                    records[if at == index { duplicate } else { at }]
                }),
                Err(Error::Checkpoint)
            );
        }
    }
}

#[test]
fn scalar_comparison_matches_real_maps_after_deletion_and_growth() {
    let mut map = Map::new(LIMIT);
    for round in 0..5 {
        map.with(|mut view| {
            view.clear();
            for (key, value) in expected(LIMIT).into_iter().rev() {
                view.insert(key, value).unwrap();
            }
            for key in (0..LIMIT as u64).step_by(round + 2) {
                view.remove(u64::MAX - key);
            }
        })
        .unwrap();
        let records =
            Map::records_from_checkpoint(&map.checkpoint(17).unwrap(), 17, LIMIT).unwrap();
        let mut native_order = Vec::new();
        map.with(|view| view.for_each(|key, value| native_order.push((key, value))))
            .unwrap();
        assert_eq!(
            verify_records(&records, native_order.len(), |index| native_order[index]),
            Ok(())
        );
    }
}
