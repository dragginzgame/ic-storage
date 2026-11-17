extern crate std;
use super::{tests::exported, *};

fn resign(bytes: &mut [u8]) {
    let digest = Sha256::digest(&bytes[..112]);
    bytes[112..].copy_from_slice(&digest);
}

#[test]
fn continuation_reopens_every_page_and_preserves_failure_position() {
    for count in [0, 1, 64, 65, 129, 4608] {
        let (manifest, pages) = exported(count);
        let mut cursor = manifest.reader().checkpoint();
        for (index, page) in pages.iter().enumerate() {
            let mut reader = Reader::resume(&manifest, &cursor).unwrap();
            assert_eq!(reader.next_page(), Some(index));
            let mut damaged = *page;
            damaged[80] ^= 1;
            assert_eq!(reader.read_page(&damaged), Err(Error::Integrity));
            assert_eq!(reader.checkpoint(), cursor);
            assert_eq!(
                reader.read_page(page).unwrap().len(),
                (count - index * 64).min(64)
            );
            cursor = reader.checkpoint();
        }
        let mut finished = Reader::resume(&manifest, &cursor).unwrap();
        assert_eq!(finished.next_page(), None);
        assert_eq!(finished.read_page(&[0; PAGE_BYTES]), Err(Error::Checkpoint));
        assert_eq!(finished.checkpoint(), cursor);
    }
}

#[test]
fn continuation_rejects_corruption_identity_and_resealed_geometry() {
    let (manifest, pages) = exported(65);
    let mut reader = manifest.reader();
    let initial = reader.checkpoint();
    reader.read_page(&pages[0]).unwrap();
    let progress = reader.checkpoint();
    reader.read_page(&pages[1]).unwrap();
    let complete = reader.checkpoint();
    for good in [initial, progress, complete] {
        for length in 0..good.len() {
            assert!(Reader::resume(&manifest, &good[..length]).is_err());
        }
        let mut extra = good.to_vec();
        extra.push(0);
        assert!(Reader::resume(&manifest, &extra).is_err());
        for index in 0..good.len() {
            let mut bad = good;
            bad[index] ^= 1;
            assert!(Reader::resume(&manifest, &bad).is_err());
        }
    }
    for (mut bytes, offset, value) in [
        (initial, 96, 1),
        (initial, 88, 1),
        (initial, 56, 99),
        (progress, 48, 3),
        (progress, 88, 0),
        (progress, 88, 2),
        (complete, 56, 1),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&u64::to_le_bytes(value));
        resign(&mut bytes);
        assert!(Reader::resume(&manifest, &bytes).is_err());
    }
    for field in [8, 16, 24, 56] {
        let mut other = manifest.encode();
        other[field] ^= 1;
        let digest = Sha256::digest(&other[..96]);
        other[96..].copy_from_slice(&digest);
        // The manifest's own decoder rejects unknown version; other changes
        // admit under their new expected identity but cannot reuse this cursor.
        if let Ok(other) = Manifest::decode(&other, word(&other[16..24]), word(&other[24..32]), 65)
        {
            assert!(Reader::resume(&other, &progress).is_err());
        }
    }
    // Even a trusted cursor must preserve cross-page ordering: its last key is
    // enforced, and refusal does not change the saved progress.
    let mut bad = progress;
    bad[96..104].copy_from_slice(&u64::MAX.to_le_bytes());
    resign(&mut bad);
    let mut reader = Reader::resume(&manifest, &bad).unwrap();
    assert_eq!(reader.read_page(&pages[1]), Err(Error::Checkpoint));
    assert_eq!(reader.checkpoint(), bad);
}

#[test]
fn independent_continuation_golden() {
    let hex = include_str!("continuation-golden.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let (manifest, pages) = exported(65);
    let mut reader = manifest.reader();
    reader.read_page(&pages[0]).unwrap();
    assert_eq!(reader.checkpoint(), bytes.as_slice());
    assert_eq!(
        Reader::resume(&manifest, &bytes).unwrap().next_page(),
        Some(1)
    );
}
