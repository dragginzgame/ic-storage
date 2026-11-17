use super::*;
use alloc::vec;
use ic_storage::ordered::{Map, Mutation};

#[test]
fn family_cursor_skips_empty_roots_and_rejects_cross_root_continuations() {
    let maps: [Map<512, 16384>; 4] = std::array::from_fn(|i| {
        let mut map = Map::new(ROOTS[i].entries);
        if i == 1 || i == 3 {
            map.with(|mut view| {
                view.insert(&[0], &vec![i as u8; ROOTS[i].value_bytes])
                    .unwrap()
            })
            .unwrap();
        }
        map
    });
    let mut pages = Vec::new();
    let manifests = std::array::from_fn(|i| {
        maps[i]
            .export_pages(1, ROOTS[i].id, |_, page| {
                pages.push(page.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap()
    });
    let family = Family::new(manifests, 1).unwrap();
    assert_eq!(family.page_count(), 2);
    assert_eq!(family.locate(0), Some((1, 0)));
    assert_eq!(family.locate(1), Some((3, 0)));
    let source = Family::DOMAINS[0];
    let target = Family::DOMAINS[1];
    let mut job = OrderedFamilyJob::initial(source)
        .unwrap()
        .seal(source, target, 1, &family)
        .unwrap();
    job.begin_import(target, source, 1).unwrap();
    let mut cursor = family.initial_cursor().unwrap();
    let mut restored: [Map<512, 16384>; 4] = std::array::from_fn(|i| Map::new(ROOTS[i].entries));
    for (index, page) in pages.iter().enumerate() {
        let mut reader = job.reader(&cursor).unwrap();
        let records = reader.read_page(page).unwrap();
        let old_cursor = cursor.clone();
        let batch = job.batch(target, source, 1, index as u32, 1).unwrap();
        let (root, _) = family.locate(index).unwrap();
        restored[root]
            .with(|mut view| {
                view.apply(
                    &records
                        .records()
                        .map(|(k, v)| Mutation::Put(k, v))
                        .collect::<Vec<_>>(),
                )
                .unwrap()
            })
            .unwrap();
        cursor = family.next_cursor(batch.end as usize, &reader).unwrap();
        job.reader(&cursor).unwrap();
        assert!(job.reader(&old_cursor).is_err());
        job = OrderedFamilyJob::decode(&job.encode().unwrap()).unwrap();
        assert!(job.batch(target, source, 1, index as u32, 1).unwrap().retry);
    }
    for (map, expected) in restored.iter().zip(maps) {
        assert_eq!(map.checkpoint(1).unwrap(), expected.checkpoint(1).unwrap());
    }
    let bytes = family.encode();
    assert!(Family::decode(&bytes[..bytes.len() - 1], 1).is_err());
    for field in 0..6 {
        let mut changed = bytes;
        changed[field * 8] ^= 1;
        assert!(Family::decode(&changed, 1).is_err());
    }
    assert!(Family::decode(&bytes, 2).is_err());
}

#[test]
fn empty_family_completes_and_retries_once() {
    let manifests = ROOTS.map(|root| {
        Map::<512, 16384>::new(root.entries)
            .export_pages(1, root.id, |_, _| -> Result<(), ()> { panic!("empty") })
            .unwrap()
    });
    let family = Family::new(manifests, 1).unwrap();
    let mut job = OrderedFamilyJob::initial(Family::DOMAINS[0])
        .unwrap()
        .seal(Family::DOMAINS[0], Family::DOMAINS[1], 1, &family)
        .unwrap();
    job.begin_import(Family::DOMAINS[1], Family::DOMAINS[0], 1)
        .unwrap();
    let cursor = family.initial_cursor().unwrap();
    job.reader(&cursor).unwrap();
    let batch = job
        .batch(Family::DOMAINS[1], Family::DOMAINS[0], 1, 0, 1)
        .unwrap();
    assert!(batch.complete && !batch.retry);
    job.reader(&cursor).unwrap();
    assert!(
        job.batch(Family::DOMAINS[1], Family::DOMAINS[0], 1, 0, 1)
            .unwrap()
            .retry
    );
}
