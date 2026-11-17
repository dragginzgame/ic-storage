use super::super::{ExportError, ImportError};
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

#[test]
fn admitted_record_stream_preserves_native_order_and_portable_bytes() {
    for count in [0, 1, 64, 65, 4608] {
        let mut map = Map::new(count);
        let mut records = Vec::new();
        map.with(|mut view| {
            for key in 0..count as u64 {
                view.insert(u64::MAX - key, key).unwrap();
            }
            if count > 1 {
                view.remove(u64::MAX);
                view.insert(0, u64::MAX).unwrap();
            }
            view.for_each(|key, value| records.push((key, value)));
        })
        .unwrap();
        let mut actual = Vec::new();
        super::super::export_records(count, 17, records.iter().copied(), |bytes| {
            assert!(bytes.len() <= 32);
            actual.extend_from_slice(bytes);
            Ok::<_, ()>(())
        })
        .unwrap();
        let mut expected = Vec::from(*b"NKEYMAP1");
        for word in [17, count as u64] {
            expected.extend_from_slice(&word.to_le_bytes());
        }
        for (k, v) in records {
            expected.extend_from_slice(&k.to_le_bytes());
            expected.extend_from_slice(&v.to_le_bytes());
        }
        expected.extend_from_slice(&Sha256::digest(&expected));
        assert_eq!(actual, expected);
        assert_eq!(actual, map.checkpoint(17).unwrap());
        assert_eq!(
            Map::records_from_checkpoint(&actual, 17, count)
                .unwrap()
                .len(),
            count
        );
    }
}

#[test]
fn wrong_stream_count_and_sink_failure_never_seal_a_checkpoint() {
    use super::super::export_records;
    for (declared, supplied) in [(0, 1), (1, 0), (2, 1), (1, 100)] {
        let mut reads = 0;
        let mut bytes = Vec::new();
        let records = (0..supplied).map(|k| {
            reads += 1;
            (k, k)
        });
        assert_eq!(
            export_records(declared, 1, records, |part| {
                bytes.extend_from_slice(part);
                Ok::<_, ()>(())
            }),
            Err(ExportError::Map(Error::Checkpoint))
        );
        assert_eq!(reads, supplied.min(declared as u64 + 1));
        assert_eq!(bytes.len(), 24 + 16 * declared.min(supplied as usize));
        assert!(Map::records_from_checkpoint(&bytes, 1, 100).is_err());
    }
    assert_eq!(
        export_records(
            usize::MAX,
            1,
            core::iter::from_fn(|| panic!("overflow read input")),
            |_| -> Result<(), ()> { panic!("overflow wrote output") }
        ),
        Err(ExportError::Map(Error::Limit))
    );
    for stop in 0usize..6 {
        let mut calls = 0;
        let mut reads = 0;
        let mut bytes = Vec::new();
        let records = (0..2).map(|k| {
            reads += 1;
            (k, k)
        });
        assert_eq!(
            export_records(2, 1, records, |part| {
                if calls == stop {
                    return Err("stop");
                }
                calls += 1;
                bytes.extend_from_slice(part);
                Ok(())
            }),
            Err(ExportError::Sink("stop"))
        );
        assert_eq!(reads, stop.saturating_sub(2).min(2));
        assert!(Map::records_from_checkpoint(&bytes, 1, 2).is_err());
    }
    let mut duplicate = Vec::new();
    export_records(2, 1, [(0, 1), (0, 1)], |part| {
        duplicate.extend_from_slice(part);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(
        Map::records_from_checkpoint(&duplicate, 1, 2),
        Err(Error::Checkpoint)
    );
}

fn resign(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn logical_records_are_sorted_and_match_native_reconstruction() {
    for count in [0, 1, 257, 4608] {
        let mut source = Map::new(count);
        source
            .with(|mut view| {
                for key in 0..count as u64 {
                    view.insert(u64::MAX - key, key ^ u64::MAX).unwrap();
                }
                if count > 1 {
                    view.remove(u64::MAX);
                    view.insert(0, u64::MAX).unwrap();
                }
            })
            .unwrap();
        let bytes = source.checkpoint(17).unwrap();
        let expected: Vec<_> = entries(&mut source).into_iter().collect();
        drop(source);
        let logical = Map::records_from_checkpoint(&bytes, 17, count).unwrap();
        assert_eq!(logical, expected);
        let mut native = Map::from_checkpoint(&bytes, 17, count).unwrap();
        assert_eq!(
            logical,
            entries(&mut native).into_iter().collect::<Vec<_>>()
        );
    }
    // The portable format permits any record order. Nonadjacent duplicates
    // refuse with either identical or conflicting values, even when re-signed.
    for duplicate_value in [10u64, 11] {
        let mut bytes = Vec::from(*b"NKEYMAP1");
        for word in [17, 3, u64::MAX, 10, 0, 20, u64::MAX, duplicate_value] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&Sha256::digest(&bytes));
        assert_eq!(
            Map::records_from_checkpoint(&bytes, 17, 3),
            Err(Error::Checkpoint)
        );
        assert!(matches!(
            Map::from_checkpoint(&bytes, 17, 3),
            Err(Error::Checkpoint)
        ));
    }
}

#[test]
fn portable_format_is_logical_and_streaming_matches_buffered_export() {
    let mut map = Map::new(8);
    map.with(|mut view| view.insert(u64::MAX, 42))
        .unwrap()
        .unwrap();
    let actual = map.checkpoint(7).unwrap();
    let mut expected = Vec::from(*b"NKEYMAP1");
    for word in [7u64, 1, u64::MAX, 42] {
        expected.extend_from_slice(&word.to_le_bytes());
    }
    expected.extend_from_slice(&Sha256::digest(&expected));
    assert_eq!(actual, expected);
    let mut streamed = Vec::new();
    map.export(7, |bytes| {
        assert!(bytes.len() <= 32);
        streamed.extend_from_slice(bytes);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(streamed, actual);
    let mut restored = Map::from_checkpoint(&streamed, 7, 1).unwrap();
    assert_eq!(entries(&mut restored), BTreeMap::from([(u64::MAX, 42)]));
    for stop in 0..5 {
        let mut calls = 0;
        assert_eq!(
            map.export(7, |_| {
                if calls == stop {
                    return Err("full");
                }
                calls += 1;
                Ok(())
            }),
            Err(ExportError::Sink("full"))
        );
        assert_eq!(map.checkpoint(7).unwrap(), actual);
    }
}

#[test]
fn malformed_or_duplicate_checkpoints_cannot_replace_a_healthy_owner() {
    let mut map = Map::new(10);
    map.with(|mut view| {
        view.insert(0, 7).unwrap();
        view.insert(1, 8).unwrap();
    })
    .unwrap();
    let saved = map.checkpoint(7).unwrap();
    for length in 0..saved.len() {
        assert!(map.recover(&saved[..length], 7).is_err());
        assert!(Map::records_from_checkpoint(&saved[..length], 7, 10).is_err());
        assert_eq!(map.checkpoint(7).unwrap(), saved);
    }
    for fault in 0..7 {
        let mut bad = saved.clone();
        let expected = match fault {
            0 => {
                bad[0] ^= 1;
                Error::Checkpoint
            }
            1 => {
                bad[8] ^= 1;
                Error::Epoch
            }
            2 => {
                bad[24] ^= 1;
                Error::Integrity
            }
            3 => {
                bad.extend_from_slice(&[0]);
                Error::Checkpoint
            }
            4 => {
                bad[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
                Error::Limit
            }
            5 => {
                bad.copy_within(24..32, 40);
                resign(&mut bad);
                Error::Checkpoint
            }
            _ => {
                bad[16..24].copy_from_slice(&1u64.to_le_bytes());
                resign(&mut bad);
                Error::Checkpoint
            }
        };
        assert_eq!(map.recover(&bad, 7), Err(expected));
        assert_eq!(Map::records_from_checkpoint(&bad, 7, 10), Err(expected));
        assert_eq!(map.checkpoint(7).unwrap(), saved);
    }
    assert!(matches!(
        Map::from_checkpoint(&saved, 7, 1),
        Err(Error::Limit)
    ));
    assert_eq!(
        Map::records_from_checkpoint(&saved, 7, 1),
        Err(Error::Limit)
    );
    let empty = Map::new(0).checkpoint(8).unwrap();
    map.recover(&empty, 8).unwrap();
    assert_eq!(map.len(), Ok(0));
    map.with(|mut view| view.insert(9, 10)).unwrap().unwrap();
}

#[test]
fn streamed_import_crosses_chunk_boundaries_and_preserves_full_width_values() {
    for count in [0, 1, 255, 256, 257, 513] {
        let mut source = Map::new(count);
        source
            .with(|mut view| {
                for key in 0..count as u64 {
                    view.insert(u64::MAX - key, key ^ u64::MAX).unwrap();
                }
            })
            .unwrap();
        let bytes = source.checkpoint(7).unwrap();
        let mut input = bytes.as_slice();
        let mut largest = 0;
        let mut restored = Map::from_reader(bytes.len(), 7, count, |buffer| {
            largest = largest.max(buffer.len());
            input.read_exact(buffer)
        })
        .unwrap();
        assert!(input.is_empty());
        assert!(largest <= 4096);
        if count >= 256 {
            assert_eq!(largest, 4096);
        }
        assert_eq!(entries(&mut restored), entries(&mut source));
        restored
            .with(|mut view| {
                if count != 0 {
                    assert_eq!(view.remove(u64::MAX), Some(u64::MAX));
                    view.insert(7, 42).unwrap();
                }
            })
            .unwrap();
    }
}

#[test]
fn streamed_import_rejects_incomplete_corrupt_and_misframed_sources() {
    let mut old = Map::new(8);
    old.with(|mut view| {
        view.insert(0, 7).unwrap();
        view.insert(1, 8).unwrap();
    })
    .unwrap();
    let saved = old.checkpoint(7).unwrap();
    for end in 0..saved.len() {
        let mut input = &saved[..end];
        assert!(matches!(
            Map::from_reader(saved.len(), 7, 8, |buffer| input.read_exact(buffer)),
            Err(ImportError::Source(_))
        ));
    }
    for (offset, error) in [
        (0, Error::Checkpoint),
        (8, Error::Epoch),
        (32, Error::Integrity),
    ] {
        let mut bad = saved.clone();
        bad[offset] ^= 1;
        let mut input = bad.as_slice();
        assert!(
            matches!(Map::from_reader(bad.len(), 7, 8, |buffer| input.read_exact(buffer)),
            Err(ImportError::Map(actual)) if actual == error)
        );
    }
    let mut duplicate = saved.clone();
    duplicate.copy_within(24..32, 40);
    resign(&mut duplicate);
    let mut input = duplicate.as_slice();
    assert!(matches!(
        Map::from_reader(duplicate.len(), 7, 8, |buffer| input.read_exact(buffer)),
        Err(ImportError::Map(Error::Checkpoint))
    ));
    for (length, limit, error) in [
        (saved.len() + 1, 8, Error::Checkpoint),
        (saved.len() - 1, 8, Error::Checkpoint),
        (saved.len(), 1, Error::Limit),
    ] {
        let mut input = saved.as_slice();
        assert!(
            matches!(Map::from_reader(length, 7, limit, |buffer| input.read_exact(buffer)),
            Err(ImportError::Map(actual)) if actual == error)
        );
        assert_eq!(input.len(), saved.len() - 24); // header only, no record reads
    }
    assert!(matches!(
        Map::from_reader(55, 7, 8, |_| Err("must not read")),
        Err(ImportError::Map(Error::Checkpoint))
    ));
    assert_eq!(old.checkpoint(7).unwrap(), saved);
}
