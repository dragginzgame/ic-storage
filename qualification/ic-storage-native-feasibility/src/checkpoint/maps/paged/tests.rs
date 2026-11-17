use super::*;
use alloc::vec::Vec;

fn sources(counts: [usize; 2]) -> [Map; 2] {
    core::array::from_fn(|id| {
        let mut map = Map::new(counts[id]);
        map.with(|mut view| {
            for key in 0..counts[id] as u64 {
                view.insert(u64::MAX - key, key + id as u64).unwrap();
            }
        })
        .unwrap();
        map
    })
}

fn exported(counts: [usize; 2], epoch: u64) -> (Manifest, [Vec<[u8; PAGE_BYTES]>; 2]) {
    let maps = sources(counts);
    let mut pages = [Vec::new(), Vec::new()];
    let manifest = export(&maps, epoch, |root, _, page| {
        pages[root as usize - 1].push(*page);
        Ok::<_, ()>(())
    })
    .unwrap();
    for root in &mut pages {
        root.reverse();
    }
    (manifest, pages)
}

fn seal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn paired_pages_preserve_both_roots_and_empty_asymmetry() {
    for counts in [[0, 0], [0, 65], [65, 0], [1, 1], [64, 65], [2304, 2304]] {
        let (manifest, pages) = exported(counts, 17);
        let admitted = Manifest::decode(&manifest.encode(), 17, 2304).unwrap();
        assert_eq!(admitted, manifest);
        assert_eq!(admitted.page_count(), pages.iter().map(Vec::len).sum());
        for id in 0..2 {
            let root = &admitted.roots()[id];
            assert_eq!(root.len(), counts[id]);
            let mut reader = root.reader();
            let mut records = Vec::new();
            while let Some(index) = reader.next_page() {
                records.extend(reader.read_page(&pages[id][index]).unwrap());
            }
            let expected: Vec<_> = (0..counts[id] as u64)
                .rev()
                .map(|k| (u64::MAX - k, k + id as u64))
                .collect();
            assert_eq!(records, expected);
        }
        if counts.iter().all(|&count| count != 0) {
            assert!(
                admitted.roots()[0]
                    .reader()
                    .read_page(&pages[1][0])
                    .is_err()
            );
        }
    }
}

#[test]
fn pair_identity_epoch_nested_integrity_and_bounds_refuse() {
    let (manifest, _) = exported([65, 1], 17);
    let good = manifest.encode();
    for length in 0..good.len() {
        assert!(Manifest::decode(&good[..length], 17, 65).is_err());
    }
    let mut extra = good.to_vec();
    extra.push(0);
    assert!(Manifest::decode(&extra, 17, 65).is_err());
    for index in 0..good.len() {
        let mut bad = good;
        bad[index] ^= 1;
        assert!(Manifest::decode(&bad, 17, 65).is_err());
    }
    assert_eq!(Manifest::decode(&good, 18, 65), Err(Error::Epoch));
    assert_eq!(Manifest::decode(&good, 17, 64), Err(Error::Limit));
    assert_eq!(
        Manifest::decode(&exported([1, 65], 17).0.encode(), 17, 64),
        Err(Error::Limit)
    );
    let mut overflow = good;
    for start in [32, 32 + ROOT_BYTES] {
        let root = &mut overflow[start..start + ROOT_BYTES];
        root[32..40].copy_from_slice(&(usize::MAX as u64).to_le_bytes());
        root[40..48].copy_from_slice(&(usize::MAX.div_ceil(64) as u64).to_le_bytes());
        seal(root);
    }
    seal(&mut overflow);
    assert_eq!(
        Manifest::decode(&overflow, 17, usize::MAX),
        Err(Error::Limit)
    );
    for mode in 0..6 {
        let mut bad = good;
        match mode {
            0 => bad[0] ^= 1,
            1 => bad[8] ^= 1,
            2 => bad[24] ^= 1,
            3 => {
                bad[32..32 + ROOT_BYTES].copy_from_slice(&good[32 + ROOT_BYTES..END]);
                bad[32 + ROOT_BYTES..END].copy_from_slice(&good[32..32 + ROOT_BYTES]);
            }
            4 => bad[32 + 56] ^= 1, // valid pair digest cannot hide bad nested digest
            _ => {
                let newer = exported([65, 1], 18).0.encode();
                bad[32 + ROOT_BYTES..END].copy_from_slice(&newer[32 + ROOT_BYTES..END]);
            }
        }
        seal(&mut bad);
        assert!(Manifest::decode(&bad, 17, 65).is_err());
    }
}

#[test]
fn second_root_export_failure_returns_no_pair_and_preserves_sources() {
    let maps = sources([65, 65]);
    let before = [
        maps[0].checkpoint(17).unwrap(),
        maps[1].checkpoint(17).unwrap(),
    ];
    let mut first_pages = 0;
    assert_eq!(
        export(&maps, 17, |root, _, _| {
            if root == 2 {
                return Err("second root unavailable");
            }
            first_pages += 1;
            Ok(())
        }),
        Err(ExportError::Sink("second root unavailable"))
    );
    assert_eq!(first_pages, 2);
    for id in 0..2 {
        assert_eq!(maps[id].checkpoint(17).unwrap(), before[id]);
    }
}

#[test]
fn independent_pair_manifest_golden() {
    let hex = include_str!("golden.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    // Independent Perl pack/Digest::SHA: two empty roots at epoch 17.
    let manifest = exported([0, 0], 17).0;
    assert_eq!(bytes, manifest.encode());
    assert_eq!(Manifest::decode(&bytes, 17, 0).unwrap(), manifest);
}
