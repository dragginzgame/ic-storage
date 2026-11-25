use super::*;

impl Directory {
    // Model-only publication; durable publication belongs to Store.
    fn publish(&mut self, prepared: Prepared) -> Result<(), Error> {
        if self.image != prepared.base || self.data_start != prepared.candidate.data_start {
            return Err(Error::Stale);
        }
        // Re-admit the candidate under this owner's budgets, even when two
        // owners began from the same image with different local limits.
        let candidate = Self::validate(prepared.candidate.image, self.budget, self.data_start)?;
        candidate.encode()?;
        *self = candidate;

        Ok(())
    }

    pub(super) fn page(&self, id: AllocationId, logical: u64) -> Option<u64> {
        self.page_span(id, logical).map(|(page, _)| page)
    }
}

fn key(i: usize) -> StableKey {
    StableKey::parse(format!("test.store{i}.v1")).unwrap()
}

#[test]
fn identities_survive_reorder_omission_and_reopen_beyond_256() {
    let budget = StorageConfig::default();
    let mut d = Directory::empty(budget, 1).unwrap();
    let p = d.prepare((0..1_024).rev().map(key).collect(), &[]).unwrap();
    d.publish(p).unwrap();
    let original = d.by_key.clone();
    let p = d.prepare(vec![key(0), key(2_000)], &[]).unwrap();
    let recovered = Directory::decode(&p.candidate.encode().unwrap(), budget, 1).unwrap();
    for (key, id) in original {
        assert_eq!(recovered.by_key[&key], id);
    }
    assert_eq!(recovered.by_key[&key(2_000)], AllocationId(1_025));
    assert_eq!(recovered.image.entries.len(), 1_025);
}

#[test]
fn interleaved_growth_maps_disjoint_ranges_and_rejects_stale_candidates() {
    let mut d = Directory::empty(StorageConfig::default(), 2).unwrap();
    d.publish(d.prepare(vec![key(0), key(1)], &[]).unwrap())
        .unwrap();
    let a = d.by_key[&key(0)];
    let b = d.by_key[&key(1)];
    let first = d.prepare(vec![], &[(a, 2), (b, 3), (a, 1)]).unwrap();
    let stale = d.prepare(vec![key(2)], &[]).unwrap();
    d.publish(first).unwrap();
    assert_eq!(
        (d.page(a, 0), d.page(a, 1), d.page(a, 2), d.page(a, 3)),
        (Some(2), Some(3), Some(7), None)
    );
    assert_eq!((d.page(b, 0), d.page(b, 2)), (Some(4), Some(6)));
    assert!(matches!(d.publish(stale), Err(Error::Stale)));
}

#[test]
fn rejected_budgets_and_overflows_preserve_the_base() {
    let budget = StorageConfig {
        allocations: 1,
        extents: 1,
        pages: 3,
        ..StorageConfig::default()
    };
    let mut d = Directory::empty(budget, 1).unwrap();
    d.publish(d.prepare(vec![key(0)], &[]).unwrap()).unwrap();
    let original = d.image.clone();
    let id = d.by_key[&key(0)];
    assert!(d.prepare(vec![key(1)], &[]).is_err());
    assert!(d.prepare(vec![], &[(id, 3)]).is_err());
    assert!(d.prepare(vec![], &[(id, u64::MAX)]).is_err());
    assert!(d.prepare(vec![], &[(id, 1), (id, 2)]).is_err());
    assert_eq!(d.image, original);
}

#[test]
fn publication_enforces_receiving_budget_and_identity_exhaustion_is_fallible() {
    let mut strict = Directory::empty(
        StorageConfig {
            allocations: 1,
            ..StorageConfig::default()
        },
        1,
    )
    .unwrap();
    let relaxed = Directory::empty(StorageConfig::default(), 1).unwrap();
    let candidate = relaxed.prepare(vec![key(0), key(1)], &[]).unwrap();
    let before = strict.image.clone();
    assert!(matches!(strict.publish(candidate), Err(Error::Budget)));
    assert_eq!(strict.image, before);

    let mut image = relaxed.image;
    image.next_id = u64::MAX - 1;
    let mut d = Directory::validate(image, StorageConfig::default(), 1).unwrap();
    d.publish(d.prepare(vec![key(0)], &[]).unwrap()).unwrap();
    assert_eq!(d.by_key[&key(0)], AllocationId(u64::MAX - 1));
    let before = d.image.clone();
    assert!(matches!(d.prepare(vec![key(1)], &[]), Err(Error::Budget)));
    assert_eq!(d.image, before);
}

#[test]
fn recovery_rejects_overlap_duplicate_keys_bounds_and_bad_format() {
    let budget = StorageConfig::default();
    let mut d = Directory::empty(budget, 1).unwrap();
    d.publish(d.prepare(vec![key(0), key(1)], &[]).unwrap())
        .unwrap();
    let ids: Vec<_> = d.by_id.keys().copied().collect();
    d.publish(d.prepare(vec![], &[(ids[0], 2), (ids[1], 2)]).unwrap())
        .unwrap();
    for mutation in 0..5 {
        let mut image = d.image.clone();
        match mutation {
            0 => image.entries[1].extents[0].start = 2,
            1 => image.entries[1].key = image.entries[0].key.clone(),
            2 => image.entries[0].extents[0].start = 0,
            3 => image.entries[0].extents[0].pages = u64::MAX,
            _ => image.entries[1].id = image.entries[0].id,
        }
        let bytes = format::encode(&image, budget.directory_bytes).unwrap();
        assert!(Directory::decode(&bytes, budget, 1).is_err());
    }
    let bytes = d.encode().unwrap();
    for end in 0..bytes.len() {
        assert!(Directory::decode(&bytes[..end], budget, 1).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Directory::decode(&trailing, budget, 1).is_err());
    assert!(
        Directory::decode(
            &bytes,
            StorageConfig {
                directory_bytes: 8,
                ..budget
            },
            1
        )
        .is_err()
    );
}

#[test]
fn v1_fixture_reopens_and_reencodes_exactly() {
    // Generation 7, next id 3, next page 4, metadata at page 1.
    // Entry 2 owns app.rows.v1 and two pages starting at page 2.
    let bytes = b"ICPDIR01\x07\0\0\0\0\0\0\0\x03\0\0\0\0\0\0\0\x04\0\0\0\0\0\0\0\
                  \x01\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\
                  \x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0\x0b\0\0\0\0\0\0\0app.rows.v1\
                  \x01\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0\x02\0\0\0\0\0\0\0";
    let directory = Directory::decode(bytes, StorageConfig::default(), 1).unwrap();
    let id = directory.by_key[&StableKey::parse("app.rows.v1").unwrap()];
    assert_eq!(id.get(), 2);
    assert_eq!(directory.page(id, 1), Some(3));
    assert_eq!(directory.encode().unwrap(), bytes);
}
