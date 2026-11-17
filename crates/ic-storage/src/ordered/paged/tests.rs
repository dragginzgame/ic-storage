extern crate std;
use super::*;
use crate::ordered::Mutation;
use alloc::{alloc::Global, vec};
use core::convert::Infallible;

fn source<const V: usize>(count: usize, length: usize) -> Map<512, V> {
    let mut source = Map::new(count);
    for key in 0..count as u64 {
        source
            .with(|mut view| view.insert(&key.to_be_bytes(), &vec![key as u8; length]))
            .unwrap()
            .unwrap();
    }
    source
}
fn export<const V: usize>(source: &Map<512, V>) -> (Manifest, Vec<Vec<u8>>) {
    let mut pages = Vec::new();
    let mut indices = Vec::new();
    let manifest = source
        .export_pages(7, 23, |index, page| {
            indices.push(index);
            pages.push(page.to_vec());
            Ok::<_, Infallible>(())
        })
        .unwrap();
    assert_eq!(
        indices,
        (0..manifest.page_count()).rev().collect::<Vec<_>>()
    );
    pages.reverse();
    (manifest, pages)
}

#[test]
fn variable_rows_and_job_values_resume_without_prior_pages() {
    for (count, length) in [
        (0, 0),
        (1, 0),
        (63, 1),
        (64, 7),
        (65, 31),
        (129, 4096),
        (32, 16384),
    ] {
        let source = source::<16384>(count, length);
        let expected = source.checkpoint(7).unwrap();
        let (manifest, pages) = export(&source);
        drop(source);
        let manifest = Manifest::decode(&manifest.encode(), 7, 23, count).unwrap();
        let mut cursor = manifest.reader::<512, 16384>().checkpoint().unwrap();
        let mut candidate = Map::<512, 16384>::new(count);
        for (index, page) in pages.into_iter().enumerate() {
            // Reopen only persisted scalar descriptors and byte-defined progress.
            candidate = Map::admit(candidate.release().unwrap(), count)
                .unwrap_or_else(|_| panic!("prefix"));
            let mut reader = Reader::<512, 16384>::resume(&manifest, &cursor).unwrap();
            assert_eq!(reader.next_page(), Some(index));
            let records = reader.read_page(&page).unwrap();
            assert!(records.len() <= PAGE_RECORDS);
            assert!(!records.is_empty());
            let mutations: Vec<_> = records
                .records()
                .map(|(k, v)| Mutation::Put(k, v))
                .collect();
            candidate
                .with(|mut view| view.apply(&mutations))
                .unwrap()
                .unwrap();
            cursor = reader.checkpoint().unwrap();
            assert!(cursor.len() <= 168 + 512);
            assert_eq!(
                reader.records_read(),
                candidate.with(|view| view.len()).unwrap()
            );
            // The consumed page is dropped; the next step cannot reread it.
        }
        let mut reader = Reader::<512, 16384>::resume(&manifest, &cursor).unwrap();
        assert_eq!(reader.next_page(), None);
        assert_eq!(reader.records_read(), count);
        assert!(reader.read_page(&vec![0; PAGE_BYTES]).is_err());
        assert_eq!(candidate.checkpoint(7).unwrap(), expected);
    }
    let mut wide = Map::<512, 4096>::new(64);
    for id in 0..64u64 {
        let mut key = [255; 512];
        key[..8].copy_from_slice(&id.to_be_bytes());
        wide.with(|mut view| view.insert(&key, &[3; 4096]))
            .unwrap()
            .unwrap();
    }
    let (manifest, pages) = export(&wide);
    let mut reader = manifest.reader::<512, 4096>();
    for page in pages {
        for (key, value) in reader.read_page(&page).unwrap().records() {
            assert_eq!((key.len(), value.len()), (512, 4096));
        }
        let cursor = reader.checkpoint().unwrap();
        assert_eq!(cursor.len(), 680);
        reader = Reader::resume(&manifest, &cursor).unwrap();
    }
    assert_eq!(reader.records_read(), 64);
    assert_eq!(reader.next_page(), None);
}

#[test]
fn page_bounds_empty_keys_and_exact_fit() {
    let mut source = Map::<0, 0>::new(1);
    source.with(|mut v| v.insert(&[], &[])).unwrap().unwrap();
    let mut page = Vec::new();
    let manifest = source
        .export_pages(0, 1, |_, b| {
            page = b.to_vec();
            Ok::<_, ()>(())
        })
        .unwrap();
    let mut reader = manifest.reader::<0, 0>();
    let records = reader.read_page(&page).unwrap();
    assert_eq!(
        records.records().collect::<Vec<_>>(),
        vec![(&[][..], &[][..])]
    );
    let cursor = reader.checkpoint().unwrap();
    assert_eq!(cursor.len(), 168);
    assert_eq!(
        Reader::<0, 0>::resume(&manifest, &cursor)
            .unwrap()
            .records_read(),
        1
    );

    let source = self::source::<32768>(2, PAGE_BYTES - HEADER - 16);
    let (manifest, pages) = export(&source);
    assert_eq!(manifest.page_count(), 2);
    let mut reader = manifest.reader::<512, 32768>();
    for page in pages {
        assert_eq!(reader.read_page(&page).unwrap().len(), 1);
    }
    assert_eq!(reader.next_page(), None);
    let too_large = self::source::<32768>(1, PAGE_BYTES - HEADER - 15);
    assert_eq!(
        too_large.export_pages(7, 23, |_, _| -> Result<(), ()> { panic!("must preflight") }),
        Err(ExportError::Map(Error::Limit))
    );
    let source = self::source::<4096>(65, 4096);
    let before = source.checkpoint(7).unwrap();
    assert_eq!(
        source.export_pages(7, 0, |_, _| Ok::<_, ()>(())),
        Err(ExportError::Map(Error::Checkpoint))
    );
    let mut calls = 0;
    assert_eq!(
        source.export_pages(7, 23, |_, _| {
            calls += 1;
            Err(9)
        }),
        Err(ExportError::Sink(9))
    );
    assert_eq!(calls, 1);
    assert_eq!(source.checkpoint(7).unwrap(), before);
}

fn seal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}
fn rechain(manifest: &Manifest, pages: &mut [Vec<u8>]) -> Manifest {
    let mut next = [0; 32];
    for page in pages.iter_mut().rev() {
        page[48..80].copy_from_slice(&next);
        next = page_digest(page);
    }
    let mut bytes = manifest.encode();
    bytes[56..88].copy_from_slice(&next);
    seal(&mut bytes);
    Manifest::decode(&bytes, 7, 23, usize::MAX).unwrap()
}

#[test]
fn corrupt_missing_mixed_and_replayed_pages_preserve_progress() {
    let source = source::<4096>(65, 4096);
    let (manifest, pages) = export(&source);
    let mut reader = manifest.reader::<512, 4096>();
    let initial = reader.checkpoint().unwrap();
    for bad in [&pages[1][..], &pages[0][..PAGE_BYTES - 1], &[][..]] {
        assert!(reader.read_page(bad).is_err());
        assert_eq!(reader.checkpoint().unwrap(), initial);
    }
    for offset in [
        0,
        8,
        16,
        24,
        32,
        40,
        48,
        79,
        80,
        87,
        88,
        95,
        96,
        105,
        PAGE_BYTES - 1,
    ] {
        let mut bad = pages[0].clone();
        bad[offset] ^= 1;
        assert!(reader.read_page(&bad).is_err());
        assert_eq!(reader.checkpoint().unwrap(), initial);
    }
    reader.read_page(&pages[0]).unwrap();
    let progress = reader.checkpoint().unwrap();
    assert!(Reader::<511, 4096>::resume(&manifest, &progress).is_err());
    assert!(Reader::<512, 4095>::resume(&manifest, &progress).is_err());
    assert!(reader.read_page(&pages[0]).is_err());
    assert_eq!(reader.checkpoint().unwrap(), progress);
    // Declared key/value bounds remain independent of checksum validity.
    for refusal in [
        manifest
            .reader::<7, 4096>()
            .read_page(&pages[0])
            .unwrap_err(),
        manifest
            .reader::<512, 4095>()
            .read_page(&pages[0])
            .unwrap_err(),
    ] {
        assert_eq!(refusal, Error::Limit);
    }
    // Invalid but correctly sealed input must fail structural admission as well.
    for (offset, value) in [
        (0, 0),
        (8, 8),
        (16, 24),
        (24, 1),
        (32, 0),
        (32, 65),
        (40, 66),
        (80, u64::MAX),
        (80, 0),
        (88, u64::MAX),
    ] {
        let mut bad = pages.clone();
        bad[0][offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        let other = rechain(&manifest, &mut bad);
        let mut reader = other.reader::<512, 4096>();
        let before = reader.checkpoint().unwrap();
        assert!(reader.read_page(&bad[0]).is_err(), "field {offset}={value}");
        assert_eq!(reader.checkpoint().unwrap(), before);
    }
    let mut bad = pages.clone();
    bad[0][PAGE_BYTES - 1] = 1;
    let other = rechain(&manifest, &mut bad);
    assert!(other.reader::<512, 4096>().read_page(&bad[0]).is_err());
    // Duplicate within a page and bytes after the declared record sequence.
    let mut duplicate = pages.clone();
    let first_key = duplicate[0][HEADER + 8..HEADER + 16].to_vec();
    let second_key = HEADER + 16 + 4096 + 8;
    duplicate[0][second_key..second_key + 8].copy_from_slice(&first_key);
    let other = rechain(&manifest, &mut duplicate);
    assert!(
        other
            .reader::<512, 4096>()
            .read_page(&duplicate[0])
            .is_err()
    );
    let mut trailing = pages.clone();
    let used = word(&trailing[0][80..88]);
    trailing[0][80..88].copy_from_slice(&(used + 1).to_le_bytes());
    let other = rechain(&manifest, &mut trailing);
    assert!(other.reader::<512, 4096>().read_page(&trailing[0]).is_err());
    // Break cross-page ordering while preserving every digest.
    let mut bad = pages.clone();
    bad[1][HEADER + 8..HEADER + 16].fill(0);
    let other = rechain(&manifest, &mut bad);
    let mut reader = other.reader::<512, 4096>();
    reader.read_page(&bad[0]).unwrap();
    let before = reader.checkpoint().unwrap();
    assert!(reader.read_page(&bad[1]).is_err());
    assert_eq!(reader.checkpoint().unwrap(), before);
}

#[test]
fn manifest_and_continuation_refuse_damage_and_resealed_geometry() {
    let (manifest, pages) = export(&source::<4096>(65, 4096));
    let encoded = manifest.encode();
    for length in 0..encoded.len() {
        assert!(Manifest::decode(&encoded[..length], 7, 23, 65).is_err());
    }
    let mut extra = encoded.to_vec();
    extra.push(0);
    assert!(Manifest::decode(&extra, 7, 23, 65).is_err());
    for offset in 0..encoded.len() {
        let mut bad = encoded;
        bad[offset] ^= 1;
        assert!(Manifest::decode(&bad, 7, 23, 65).is_err());
    }
    for (offset, value) in [(8, 2), (24, 0), (32, 0), (40, 0), (40, 66), (48, 4096)] {
        let mut bad = encoded;
        bad[offset..offset + 8].copy_from_slice(&u64::to_le_bytes(value));
        seal(&mut bad);
        assert!(Manifest::decode(&bad, 7, 23, 65).is_err());
    }
    assert_eq!(Manifest::decode(&encoded, 8, 23, 65), Err(Error::Epoch));
    assert_eq!(Manifest::decode(&encoded, 7, 23, 64), Err(Error::Limit));
    let mut reader = manifest.reader::<512, 4096>();
    let initial = reader.checkpoint().unwrap();
    reader.read_page(&pages[0]).unwrap();
    let progress = reader.checkpoint().unwrap();
    for page in &pages[1..] {
        reader.read_page(page).unwrap();
    }
    let completed = reader.checkpoint().unwrap();
    for good in [&initial, &progress, &completed] {
        let mut extra = good.clone();
        extra.push(0);
        assert!(Reader::<512, 4096>::resume(&manifest, &extra).is_err());
        for length in 0..good.len() {
            assert!(Reader::<512, 4096>::resume(&manifest, &good[..length]).is_err());
        }
        for offset in 0..good.len() {
            let mut bad = good.clone();
            bad[offset] ^= 1;
            assert!(Reader::<512, 4096>::resume(&manifest, &bad).is_err());
        }
    }
    for (cursor, offset, value) in [
        (&initial, 56, 1),
        (&initial, 104, 1),
        (&progress, 48, u64::MAX),
        (&progress, 56, 0),
        (&progress, 56, 65),
        (&progress, 96, 0),
        (&progress, 104, 0),
        (&completed, 64, 9),
        (&completed, 56, 64),
    ] {
        let mut bad = cursor.clone();
        bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        seal(&mut bad);
        assert!(Reader::<512, 4096>::resume(&manifest, &bad).is_err());
    }
    let mut other = encoded;
    other[16..24].copy_from_slice(&8u64.to_le_bytes());
    seal(&mut other);
    let other = Manifest::decode(&other, 8, 23, 65).unwrap();
    assert!(Reader::<512, 4096>::resume(&other, &progress).is_err());
}

#[test]
fn independent_empty_record_golden_and_retry_after_capacity_refusal() {
    // Independently frame one empty key/value, including every canonical zero.
    let mut page = vec![0; PAGE_BYTES];
    page[..8].copy_from_slice(b"ORDPG001");
    for (offset, value) in [(8, 7u64), (16, 23), (32, 1), (40, 1), (80, 8)] {
        page[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    let mut hash = Sha256::new();
    hash.update(b"ORDPGH01");
    hash.update(&page);
    let mut expected = [0; 128];
    expected[..8].copy_from_slice(b"ORDPAG01");
    for (offset, value) in [(8, 1u64), (16, 7), (24, 23), (32, 1), (40, 1), (48, 32768)] {
        expected[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    expected[56..88].copy_from_slice(&hash.finalize());
    expected[88..96].copy_from_slice(b"ORDPDONE");
    seal(&mut expected);
    let mut source = Map::<512, 4096>::new(1);
    source
        .with(|mut view| view.insert(&[], &[]))
        .unwrap()
        .unwrap();
    let (manifest, pages) = export(&source);
    assert_eq!(pages, vec![page]);
    assert_eq!(manifest.encode(), expected);
    let mut cursor = manifest.reader::<512, 4096>().checkpoint().unwrap();
    let original = cursor.clone();
    let mut candidate = Map::<512, 4096>::new(0);
    let mut staged = Reader::<512, 4096>::resume(&manifest, &cursor).unwrap();
    let page = staged.read_page(&pages[0]).unwrap();
    let mutations: Vec<_> = page.records().map(|(k, v)| Mutation::Put(k, v)).collect();
    assert_eq!(
        candidate.with(|mut v| v.apply(&mutations)).unwrap(),
        Err(Error::Limit)
    );
    assert_eq!(cursor, original);
    assert_eq!(candidate.with(|v| v.len()).unwrap(), 0);
    candidate = Map::new_in(1, Global);
    let mut staged = Reader::<512, 4096>::resume(&manifest, &cursor).unwrap();
    let page = staged.read_page(&pages[0]).unwrap();
    let mutations: Vec<_> = page.records().map(|(k, v)| Mutation::Put(k, v)).collect();
    candidate
        .with(|mut v| v.apply(&mutations))
        .unwrap()
        .unwrap();
    cursor = staged.checkpoint().unwrap();
    let mut expected_cursor = vec![0; 168];
    expected_cursor[..8].copy_from_slice(b"ORDCUR01");
    expected_cursor[16..48].copy_from_slice(&expected[96..]);
    expected_cursor[112..120].copy_from_slice(b"ORDCDONE");
    for (offset, value) in [
        (8, 1u64),
        (48, 1),
        (56, 1),
        (104, 1),
        (120, 512),
        (128, 4096),
    ] {
        expected_cursor[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    seal(&mut expected_cursor);
    assert_eq!(cursor, expected_cursor);
    assert_eq!(
        Reader::<512, 4096>::resume(&manifest, &cursor)
            .unwrap()
            .next_page(),
        None
    );
    assert_eq!(
        candidate.checkpoint(7).unwrap(),
        source.checkpoint(7).unwrap()
    );
}
