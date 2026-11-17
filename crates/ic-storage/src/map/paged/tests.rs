extern crate std;

use super::*;
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

const VALUE_MASK: u64 = 0xa55a_0123_4567_89ab;

fn source(count: usize) -> Map {
    let mut map = Map::new(count);
    map.with(|mut view| {
        for key in 0..count as u64 {
            view.insert(u64::MAX - key, key ^ VALUE_MASK).unwrap();
        }
    })
    .unwrap();
    map
}

pub(super) fn exported(count: usize) -> (Manifest, Vec<[u8; PAGE_BYTES]>) {
    let map = source(count);
    let mut pages = Vec::new();
    let mut next = count.div_ceil(PAGE_RECORDS);
    let manifest = map
        .export_pages(17, 9, |index, page| {
            next -= 1;
            assert_eq!(index, next);
            pages.push(*page);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(next, 0);
    pages.reverse();
    (manifest, pages)
}

fn resign_manifest(bytes: &mut [u8]) {
    let digest = Sha256::digest(&bytes[..96]);
    bytes[96..].copy_from_slice(&digest);
}

// Recommit deliberately malformed logical data, so tests reach semantic
// admission rather than stopping at the first digest mismatch.
fn recommit(manifest: &mut Manifest, pages: &mut [[u8; PAGE_BYTES]]) {
    let mut next: Option<[u8; 32]> = None;
    for page in pages.iter_mut().rev() {
        if let Some(hash) = next {
            page[48..80].copy_from_slice(&hash);
        }
        next = Some(page_digest(page));
    }
    manifest.first = next.unwrap();
}

#[test]
fn paged_roundtrips_are_bounded_ordered_and_independent_of_native_layout() {
    for count in [0, 1, 63, 64, 65, 127, 128, 129, 4608] {
        let (manifest, pages) = exported(count);
        let manifest = Manifest::decode(&manifest.encode(), 17, 9, count).unwrap();
        assert_eq!(manifest.len(), count);
        assert_eq!(manifest.is_empty(), count == 0);
        assert_eq!(manifest.page_count(), pages.len());
        let mut reader = manifest.reader();
        let mut candidate = Map::new(count);
        while let Some(index) = reader.next_page() {
            let records = reader.read_page(&pages[index]).unwrap();
            assert!(records.len() <= PAGE_RECORDS);
            candidate
                .with(|mut view| {
                    for (key, value) in records {
                        assert!(view.insert(key, value).unwrap().is_none());
                    }
                })
                .unwrap();
        }
        let mut actual = BTreeMap::new();
        candidate
            .with(|view| {
                view.for_each(|key, value| {
                    actual.insert(key, value);
                })
            })
            .unwrap();
        let expected: BTreeMap<_, _> = (0..count as u64)
            .map(|k| (u64::MAX - k, k ^ VALUE_MASK))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(reader.read_page(&[0; PAGE_BYTES]), Err(Error::Checkpoint));
    }
    let mut extremes = Map::new(4);
    extremes
        .with(|mut view| {
            for (key, value) in [(u64::MAX, 0), (0, u64::MAX), (1, 7), (u64::MAX - 1, 11)] {
                view.insert(key, value).unwrap();
            }
        })
        .unwrap();
    let mut page = [0; PAGE_BYTES];
    let manifest = extremes
        .export_pages(17, 9, |_, bytes| {
            page = *bytes;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        manifest.reader().read_page(&page).unwrap(),
        [(0, u64::MAX), (1, 7), (u64::MAX - 1, 11), (u64::MAX, 0)]
    );
}

#[test]
fn independent_paged_format_golden() {
    // Independently encoded with Perl pack/Digest::SHA from the byte contract.
    // Empty manifest, then a 65-record manifest and pages in logical order.
    let hex = include_str!("golden.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let (empty, _) = exported(0);
    assert_eq!(empty.encode(), bytes[..MANIFEST_BYTES]);
    let (manifest, pages) = exported(65);
    assert_eq!(bytes.len(), 2 * MANIFEST_BYTES + 2 * PAGE_BYTES);
    assert_eq!(manifest.encode(), bytes[MANIFEST_BYTES..2 * MANIFEST_BYTES]);
    assert_eq!(
        Manifest::decode(&bytes[MANIFEST_BYTES..2 * MANIFEST_BYTES], 17, 9, 65).unwrap(),
        manifest
    );
    let mut reader = manifest.reader();
    for (index, page) in pages.iter().enumerate() {
        let start = 2 * MANIFEST_BYTES + index * PAGE_BYTES;
        assert_eq!(page.as_slice(), &bytes[start..start + PAGE_BYTES]);
        reader.read_page(page).unwrap();
    }
    assert_eq!(reader.next_page(), None);
}

#[test]
fn manifest_admission_rejects_unknown_malformed_and_unbounded_inputs() {
    let (manifest, _) = exported(65);
    let bytes = manifest.encode();
    for length in 0..bytes.len() {
        assert!(Manifest::decode(&bytes[..length], 17, 9, 65).is_err());
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(Manifest::decode(&extra, 17, 9, 65).is_err());
    for index in 0..bytes.len() {
        let mut bad = bytes;
        bad[index] ^= 1;
        assert!(Manifest::decode(&bad, 17, 9, 65).is_err());
    }
    assert_eq!(Manifest::decode(&bytes, 18, 9, 65), Err(Error::Epoch));
    assert_eq!(Manifest::decode(&bytes, 17, 8, 65), Err(Error::Checkpoint));
    assert_eq!(Manifest::decode(&bytes, 17, 0, 65), Err(Error::Checkpoint));
    assert_eq!(Manifest::decode(&bytes, 17, 9, 64), Err(Error::Limit));
    for offset in [0, 8, 40, 48, 88] {
        let mut bad = bytes;
        bad[offset] ^= 1;
        resign_manifest(&mut bad);
        assert!(Manifest::decode(&bad, 17, 9, 65).is_err());
    }
    let mut empty = exported(0).0.encode();
    empty[56] = 1;
    resign_manifest(&mut empty);
    assert_eq!(Manifest::decode(&empty, 17, 9, 65), Err(Error::Checkpoint));
    assert_eq!(
        Manifest::decode(&[0; MANIFEST_BYTES], 17, 9, 65),
        Err(Error::Checkpoint)
    );
}

#[test]
fn corrupted_missing_replayed_and_mixed_pages_do_not_advance_the_reader() {
    let (manifest, pages) = exported(129);
    let mut reader = manifest.reader();
    for length in 0..PAGE_BYTES {
        assert!(reader.read_page(&pages[0][..length]).is_err());
        assert_eq!(reader.next_page(), Some(0));
    }
    for offset in 0..PAGE_BYTES {
        let mut bad = pages[0];
        bad[offset] ^= 1;
        assert_eq!(reader.read_page(&bad), Err(Error::Integrity));
        assert_eq!(reader.next_page(), Some(0));
    }
    let mut extra = pages[0].to_vec();
    extra.push(0);
    assert_eq!(reader.read_page(&extra), Err(Error::Checkpoint));
    for (epoch, root) in [(18, 9), (17, 8)] {
        let mut foreign = Vec::new();
        source(129)
            .export_pages(epoch, root, |_, page| {
                foreign.push(*page);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            reader.read_page(foreign.last().unwrap()),
            Err(Error::Integrity)
        );
    }
    reader.read_page(&pages[0]).unwrap();
    assert_eq!(reader.read_page(&pages[2]), Err(Error::Integrity));
    assert_eq!(reader.next_page(), Some(1));
    reader.read_page(&pages[1]).unwrap();
    assert_eq!(reader.read_page(&pages[1]), Err(Error::Integrity));
    assert_eq!(reader.next_page(), Some(2)); // absent last page cannot complete
    reader.read_page(&pages[2]).unwrap();
    assert_eq!(reader.next_page(), None);
    assert_eq!(reader.read_page(&pages[2]), Err(Error::Checkpoint));
}

#[test]
fn resigned_pages_still_require_identity_geometry_order_and_terminal_seal() {
    for fault in 0..11 {
        let (mut manifest, mut pages) = exported(65);
        let bad_index = match fault {
            0..=5 => {
                pages[0][fault * 8] ^= 1;
                0
            }
            6 => {
                pages[0].copy_within(80..88, 96);
                0
            } // duplicate within page
            7 => {
                pages[1][96] = 1;
                1
            } // noncanonical padding
            8 => {
                pages[1][48] = 1;
                1
            } // final page links further
            9 => {
                let last: [u8; 8] = pages[0][80 + 63 * 16..88 + 63 * 16].try_into().unwrap();
                pages[1][80..88].copy_from_slice(&last);
                1
            }
            _ => {
                pages[1][80..88].fill(0);
                1
            } // reverse order across boundary
        };
        recommit(&mut manifest, &mut pages);
        let admitted = Manifest::decode(&manifest.encode(), 17, 9, 65).unwrap();
        let mut reader = admitted.reader();
        for page in &pages[..bad_index] {
            reader.read_page(page).unwrap();
        }
        assert_eq!(reader.read_page(&pages[bad_index]), Err(Error::Checkpoint));
        assert_eq!(reader.next_page(), Some(bad_index));
    }
}

#[test]
fn failed_exports_preserve_the_source_and_return_no_manifest() {
    let mut map = source(129);
    let saved = map.checkpoint(17).unwrap();
    for stop in 0..3 {
        let mut calls = 0;
        assert_eq!(
            map.export_pages(17, 9, |_, _| {
                if calls == stop {
                    return Err("sink full");
                }
                calls += 1;
                Ok(())
            }),
            Err(ExportError::Sink("sink full"))
        );
        assert_eq!(map.checkpoint(17).unwrap(), saved);
    }
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = map.export_pages::<()>(17, 9, |_, _| panic!("sink unwound"));
        }))
        .is_err()
    );
    assert_eq!(map.checkpoint(17).unwrap(), saved);
    assert_eq!(
        map.export_pages(17, 0, |_, _| Ok::<_, ()>(())),
        Err(ExportError::Map(Error::Checkpoint))
    );
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let _ = map.with(|_| panic!("poison"));
    }));
    assert_eq!(
        map.export_pages(17, 9, |_, _| Ok::<_, ()>(())),
        Err(ExportError::Map(Error::Poisoned))
    );
}
