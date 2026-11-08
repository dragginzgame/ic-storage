use super::*;

fn key(i: usize) -> StableKey {
    StableKey::parse(format!("test.store{i}.v1")).unwrap()
}

#[test]
fn identities_survive_reorder_omission_and_reopen_beyond_256() {
    let budget = StorageConfig::default();
    let mut d = Directory::empty(budget, 1).unwrap();
    d.apply((0..1_024).rev().map(key).collect(), &[]).unwrap();
    let original = d.by_key.clone();
    d.apply(vec![key(0), key(2_000)], &[]).unwrap();
    let recovered = Directory::decode(&d.image.encode(budget).unwrap(), budget, 1).unwrap();
    for (key, id) in original {
        assert_eq!(recovered.by_key[&key], id);
    }
    assert_eq!(recovered.by_key[&key(2_000)], AllocationId(1_025));
    assert_eq!(recovered.image.entries.len(), 1_025);
}

#[test]
fn interleaved_growth_maps_disjoint_ranges() {
    let mut d = Directory::empty(StorageConfig::default(), 2).unwrap();
    d.apply(vec![key(0), key(1)], &[]).unwrap();
    let a = d.by_key[&key(0)];
    let b = d.by_key[&key(1)];
    d.apply(vec![], &[(a, 2), (b, 3), (a, 1)]).unwrap();
    assert_eq!(
        (d.page(a, 0), d.page(a, 1), d.page(a, 2), d.page(a, 3)),
        (Some(2), Some(3), Some(7), None)
    );
    assert_eq!((d.page(b, 0), d.page(b, 2)), (Some(4), Some(6)));
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
    d.apply(vec![key(0)], &[]).unwrap();
    let original = d.image.clone();
    let id = d.by_key[&key(0)];
    assert!(d.apply(vec![key(1)], &[]).is_err());
    assert!(d.apply(vec![], &[(id, 3)]).is_err());
    assert!(d.apply(vec![], &[(id, u64::MAX)]).is_err());
    assert!(d.apply(vec![], &[(id, 1), (id, 2)]).is_err());
    assert_eq!(d.image, original);
}

#[test]
fn mutation_enforces_owner_budget_and_identity_exhaustion_is_fallible() {
    let mut strict = Directory::empty(
        StorageConfig {
            allocations: 1,
            ..StorageConfig::default()
        },
        1,
    )
    .unwrap();
    let relaxed = Directory::empty(StorageConfig::default(), 1).unwrap();
    let before = strict.image.clone();
    assert!(matches!(
        strict.apply(vec![key(0), key(1)], &[]),
        Err(Error::Budget)
    ));
    assert_eq!(strict.image, before);

    let mut image = relaxed.image;
    image.next_id = u64::MAX - 1;
    let mut d = Directory::validate(image, StorageConfig::default(), 1).unwrap();
    d.apply(vec![key(0)], &[]).unwrap();
    assert_eq!(d.by_key[&key(0)], AllocationId(u64::MAX - 1));
    let before = d.image.clone();
    assert!(matches!(d.apply(vec![key(1)], &[]), Err(Error::Budget)));
    assert_eq!(d.image, before);
}

#[test]
fn recovery_rejects_overlap_duplicate_keys_bounds_and_bad_wire() {
    let budget = StorageConfig::default();
    let mut d = Directory::empty(budget, 1).unwrap();
    d.apply(vec![key(0), key(1)], &[]).unwrap();
    let ids: Vec<_> = d.image.entries.iter().map(|entry| entry.id).collect();
    d.apply(vec![], &[(ids[0], 2), (ids[1], 2)]).unwrap();
    for mutation in 0..5 {
        let mut image = d.image.clone();
        match mutation {
            0 => image.entries[1].extents[0].start = 2,
            1 => image.entries[1].key = image.entries[0].key.clone(),
            2 => image.entries[0].extents[0].start = 0,
            3 => image.entries[0].extents[0].pages = u64::MAX,
            _ => image.entries[1].id = image.entries[0].id,
        }
        let payload = ic_wire::native::encode(&image, budget.wire).unwrap();
        let bytes = [FRAME, &payload].concat();
        assert!(Directory::decode(&bytes, budget, 1).is_err());
    }
    let bytes = d.image.encode(budget).unwrap();
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
                wire: ic_wire::Limits {
                    bytes: 8,
                    ..budget.wire
                },
                ..budget
            },
            1
        )
        .is_err()
    );
}

// Private fixture helpers; excluded from production compilation.
impl Directory {
    fn apply(&mut self, keys: Vec<StableKey>, growth: &[(AllocationId, u64)]) -> Result<(), Error> {
        let image = self.candidate_image(keys, growth, self.image.next_page)?;
        let candidate = Self::validate(image, self.budget, 1)?;
        candidate.image.encode(self.budget)?;
        *self = candidate;
        Ok(())
    }

    fn page_span(&self, id: AllocationId, logical: u64) -> Option<(u64, u64)> {
        let index = self.entry_index(id).ok()?;
        let ends = &self.logical_ends[index];
        let slot = ends.partition_point(|&end| end <= logical);
        let extent = self.image.entries[index].extents.get(slot)?;
        let start = if slot == 0 { 0 } else { ends[slot - 1] };
        Some((extent.start + logical - start, ends[slot] - logical))
    }

    pub(super) fn page(&self, id: AllocationId, logical: u64) -> Option<u64> {
        self.page_span(id, logical).map(|(page, _)| page)
    }
}

#[test]
fn sparse_opaque_ids_reopen_and_grow_the_correct_entries() {
    let budget = StorageConfig::default();
    let mut directory = Directory::empty(budget, 1).unwrap();
    directory.apply(vec![key(0), key(1), key(2)], &[]).unwrap();
    let ids = [7, 1_000, u64::MAX - 2];
    for (entry, id) in directory.image.entries.iter_mut().zip(ids) {
        entry.id = AllocationId(id);
    }
    directory.image.next_id = u64::MAX - 1;
    let mut recovered =
        Directory::decode(&directory.image.encode(budget).unwrap(), budget, 1).unwrap();
    for (index, id) in ids.into_iter().enumerate() {
        let id = AllocationId(id);
        assert_eq!(recovered.by_key[&key(index)], id);
        recovered.apply(vec![], &[(id, 1)]).unwrap();
        assert_eq!(recovered.page(id, 0), Some(1 + index as u64));
        assert_eq!(recovered.page(id, 1), None);
    }
    let before = recovered.image.clone();
    for id in [0, 6, 8, 999, 1_001, u64::MAX - 1, u64::MAX] {
        assert!(matches!(
            recovered.apply(vec![], &[(AllocationId(id), 0)]),
            Err(Error::Missing)
        ));
        assert_eq!(recovered.image, before);
    }
    for mutation in 0..2 {
        let mut invalid = before.clone();
        if mutation == 0 {
            invalid.entries.swap(0, 1);
        } else {
            invalid.entries[1].id = invalid.entries[0].id;
        }
        assert!(Directory::decode(&invalid.encode(budget).unwrap(), budget, 1).is_err());
    }
}

// Explicit native diagnostic; timings are not an IC instruction/page-charge model.
#[test]
#[ignore = "manual optimized native comparison of the two ID lookup representations"]
fn compare_id_lookup_representations() {
    use std::{hint::black_box, time::Instant};
    for count in [1, 32, 256, 1_024, 4_096] {
        let mut directory = Directory::empty(StorageConfig::default(), 1).unwrap();
        directory.apply((0..count).map(key).collect(), &[]).unwrap();
        for (index, entry) in directory.image.entries.iter_mut().enumerate() {
            entry.id = AllocationId(2 * index as u64 + 1);
        }
        directory.image.next_id = 2 * count as u64 + 1;
        let directory = Directory::validate(directory.image, StorageConfig::default(), 1).unwrap();
        let tree: BTreeMap<_, _> = directory
            .image
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.id, index))
            .collect();
        let mut seed = 1_u64;
        let queries: Vec<_> = (0..200_000)
            .map(|_| {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                AllocationId((seed >> 32) % (2 * count as u64 + 1))
            })
            .collect();
        let start = Instant::now();
        let mut old_sum = 0_usize;
        for &id in &queries {
            old_sum = old_sum.wrapping_add(
                black_box(&tree)
                    .get(&black_box(id))
                    .copied()
                    .unwrap_or(count),
            );
        }
        let tree_time = start.elapsed();
        let start = Instant::now();
        let mut new_sum = 0_usize;
        for &id in &queries {
            new_sum = new_sum.wrapping_add(
                black_box(&directory)
                    .entry_index(black_box(id))
                    .unwrap_or(count),
            );
        }
        let vector_time = start.elapsed();
        assert_eq!(old_sum, new_sum);
        println!(
            "allocations={count},lookups={},tree_ns={},entries_ns={}",
            queries.len(),
            tree_time.as_nanos(),
            vector_time.as_nanos()
        );
    }
}
