use super::*;

fn sample(native: bool) -> Sample {
    Sample {
        native,
        rows: 192,
        other: 0,
        seed: 1,
        batch: 8,
        operation: 1,
        instructions: if native { 20 } else { 40 },
        units: 8,
        memory: [100, 0, 0],
    }
}

#[test]
fn reports_refuse_unpaired_or_duplicate_observations() {
    assert!(report::render(&[sample(true)], &[1], "test").is_err());
    assert!(report::render(&[sample(true), sample(true), sample(false)], &[1], "test").is_err());
    let text = report::render(&[sample(true), sample(false)], &[1], "test").unwrap();
    assert!(text.contains("| 20 | 40 | 0.500 |"));
}

#[test]
fn oracle_preserves_population_through_churn_and_counts_scan_entries() {
    let mut map = (0..64).map(|key| (key, key + 1000)).collect();
    for operation in 0..8 {
        expected(&mut map, 64, 3, 64, operation);
    }
    assert_eq!(map.len(), 128);
    assert_eq!(
        map.keys().copied().collect::<Vec<_>>(),
        (64..192).collect::<Vec<_>>()
    );
    assert_eq!(
        expected(&mut map, 64, 3, 64, 8),
        (64..192).map(|key| key + 1000).sum::<u64>()
    );
}
