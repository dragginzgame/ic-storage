use super::*;
use crate::checkpoint::maps::paged;
use alloc::vec::Vec;
use ic_storage::map::paged::PAGE_BYTES;

const A: u64 = 131072;
const B: u64 = 393216;

pub(super) struct Fixture {
    pub(super) job: PagedMapJob,
    pub(super) readers: [Reader; 2],
    pub(super) maps: [Map; 2],
    pub(super) pages: [Vec<[u8; PAGE_BYTES]>; 2],
}

pub(super) fn fixture(counts: [usize; 2]) -> Fixture {
    let sources = counts.map(|count| {
        let mut map = Map::new(count);
        map.with(|mut view| {
            for key in 0..count as u64 {
                view.insert(key, key ^ 99).unwrap();
            }
        })
        .unwrap();
        map
    });
    let mut pages = [Vec::new(), Vec::new()];
    let manifest = paged::export(&sources, 1, |root, _, bytes| {
        pages[root as usize - 1].push(*bytes);
        Ok::<_, ()>(())
    })
    .unwrap();
    for chain in &mut pages {
        chain.reverse();
    }
    let mut job = PagedMapJob::initial(A)
        .unwrap()
        .seal(A, B, 1, &manifest)
        .unwrap();
    job.begin_import(B, A, 1).unwrap();
    Fixture {
        job,
        readers: manifest.roots().each_ref().map(|root| root.reader()),
        maps: counts.map(Map::new),
        pages,
    }
}

pub(super) fn step(
    job: &mut PagedMapJob,
    readers: &mut [Reader; 2],
    maps: &mut [Map; 2],
    pages: &[Vec<[u8; PAGE_BYTES]>; 2],
    start: u32,
) -> bool {
    let batch = job.batch(B, A, 1, start, 1).unwrap();
    for global in batch.start as usize..batch.end as usize {
        let root = usize::from(global >= pages[0].len());
        let index = readers[root].next_page().unwrap();
        let records = readers[root].read_page(&pages[root][index]).unwrap();
        maps[root]
            .with(|mut view| {
                for (key, value) in records {
                    assert!(view.insert(key, value).unwrap().is_none());
                }
            })
            .unwrap();
    }
    batch.complete
}

#[test]
fn continuation_binds_native_prefix_and_reopens_each_job_batch() {
    for counts in [[0, 0], [0, 65], [65, 0], [129, 65], [2304, 2304]] {
        let Fixture {
            mut job,
            mut readers,
            mut maps,
            pages,
        } = fixture(counts);
        let mut bytes = encode(&job, &readers, &maps).unwrap();
        for start in 0.. {
            readers = decode(&bytes, &job, &maps).unwrap();
            let complete = step(&mut job, &mut readers, &mut maps, &pages, start);
            bytes = encode(&job, &readers, &maps).unwrap();
            job = PagedMapJob::decode(&job.encode().unwrap()).unwrap();
            assert!(decode(&bytes, &job, &maps).is_ok());
            if complete {
                break;
            }
        }
        assert_eq!(maps.each_ref().map(|map| map.len().unwrap()), counts);
    }
}

#[test]
fn continuation_rejects_corrupt_mixed_stale_and_changed_native_state() {
    let Fixture {
        mut job,
        mut readers,
        mut maps,
        pages,
    } = fixture([129, 65]);
    let initial = encode(&job, &readers, &maps).unwrap();
    step(&mut job, &mut readers, &mut maps, &pages, 0);
    let good = encode(&job, &readers, &maps).unwrap();
    assert!(decode(&initial, &job, &maps).is_err());
    for length in 0..good.len() {
        assert!(decode(&good[..length], &job, &maps).is_err());
    }
    for index in 0..good.len() {
        let mut bad = good;
        bad[index] ^= 1;
        assert!(decode(&bad, &job, &maps).is_err());
    }
    let mut extra = good.to_vec();
    extra.push(0);
    assert!(decode(&extra, &job, &maps).is_err());
    for (from, to) in [(16, 120), (CURSORS, PREFIXES), (PREFIXES, SEAL)] {
        let mut mixed = good;
        mixed[from..to].copy_from_slice(&initial[from..to]);
        let digest = Sha256::digest(&mixed[..SEAL + 8]);
        mixed[SEAL + 8..].copy_from_slice(&digest);
        assert!(decode(&mixed, &job, &maps).is_err());
    }
    maps[0]
        .with(|mut view| view.insert(0, 1234).unwrap())
        .unwrap();
    assert!(decode(&good, &job, &maps).is_err());
    maps[0]
        .with(|mut view| view.insert(0, 99).unwrap())
        .unwrap();
    assert!(decode(&good, &job, &maps).is_ok());
    // A valid job with a different last request cannot borrow this continuation.
    let mut other = fixture([129, 65]).job;
    other.batch(B, A, 1, 0, 2).unwrap();
    assert!(decode(&good, &other, &maps).is_err());
    job.restart_import(B, A, 1).unwrap();
    assert!(decode(&good, &job, &maps).is_err());
    let Fixture {
        readers: fresh,
        maps: empty,
        ..
    } = fixture([129, 65]);
    assert!(encode(&job, &readers, &empty).is_err());
    assert!(encode(&job, &fresh, &maps).is_err());
    let restart = encode(&job, &fresh, &empty).unwrap();
    assert!(decode(&restart, &job, &empty).is_ok());
}

#[test]
fn consumed_binding_preserves_later_native_writes() {
    let Fixture {
        mut job,
        mut readers,
        mut maps,
        pages,
    } = fixture([1, 1]);
    step(&mut job, &mut readers, &mut maps, &pages, 0);
    assert!(step(&mut job, &mut readers, &mut maps, &pages, 1));
    let bytes = encode(&job, &readers, &maps).unwrap();
    assert!(encode(&job, &readers, &[Map::new(1), Map::new(1)]).is_err());
    maps[0]
        .with(|mut view| view.insert(0, 1234).unwrap())
        .unwrap();
    assert!(job.batch(B, A, 1, 1, 1).unwrap().retry);
    assert!(decode(&bytes, &job, &maps).is_ok());
    assert!(job.restart_import(B, A, 1).is_err());
}

#[test]
fn independent_empty_consumed_continuation_golden() {
    let Fixture {
        mut job,
        readers,
        maps,
        ..
    } = fixture([0, 0]);
    job.batch(B, A, 1, 0, 2).unwrap();
    let hex = include_str!("golden.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(encode(&job, &readers, &maps).unwrap(), bytes.as_slice());
    assert!(decode(&bytes, &job, &maps).is_ok());
}
