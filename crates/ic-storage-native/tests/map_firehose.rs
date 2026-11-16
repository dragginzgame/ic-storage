//! Sustained load through the public native collection API, not fixture internals.
#![cfg(feature = "map")]
#![forbid(unsafe_code)]

use ic_storage_native::map::{Error, Map};
use std::{
    fs::{self, File, OpenOptions},
    hint::black_box,
    io::{BufWriter, Read, Write},
    time::Instant,
};

const RECORDS: u64 = 1_000_000;
const BATCH: u64 = 4096;
const ROUNDS: u64 = 64;

fn key(index: u64) -> u64 {
    index.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(17)
}
fn value(index: u64, round: u64) -> u64 {
    key(index) ^ round.wrapping_mul(0xd6e8_feb8_6659_fd93)
}

fn report(phase: &str, operations: u64, start: Instant) {
    let elapsed = start.elapsed();
    println!(
        "phase={phase} operations={operations} seconds={:.6} ops_per_second={:.0}",
        elapsed.as_secs_f64(),
        operations as f64 / elapsed.as_secs_f64()
    );
}

#[test]
#[ignore = "sustained million-key workload; run make native-map-firehose"]
fn million_keys_mixed_load_native_reopen_and_portable_recovery() {
    let mut map = Map::new(RECORDS as usize);
    let start = Instant::now();
    for first in (0..RECORDS).step_by(BATCH as usize) {
        let end = (first + BATCH).min(RECORDS);
        map.with(|mut view| {
            view.reserve((end - first) as usize).unwrap();
            for index in first..end {
                assert_eq!(
                    view.insert(black_box(key(index)), value(index, 0)),
                    Ok(None)
                );
            }
        })
        .unwrap();
    }
    report("ingest", RECORDS, start);
    assert_eq!(map.len(), Ok(RECORDS as usize));
    let allocated = map.allocated_bytes().unwrap();
    println!(
        "records={RECORDS} slot_bytes={allocated} effective_capacity={}",
        map.capacity().unwrap()
    );
    assert_eq!(
        map.with(|mut view| view.insert(key(RECORDS), 0)),
        Ok(Err(Error::Limit))
    );

    let start = Instant::now();
    for round in 1..=ROUNDS {
        for first in (0..RECORDS).step_by(BATCH as usize) {
            let end = (first + BATCH).min(RECORDS);
            map.with(|mut view| {
                for index in first..end {
                    assert_eq!(view.get(key(index)), Some(value(index, round - 1)));
                    assert_eq!(
                        view.insert(key(index), value(index, round)),
                        Ok(Some(value(index, round - 1)))
                    );
                }
                for index in (first..end).filter(|index| index % 2 == 0) {
                    assert_eq!(view.remove(key(index)), Some(value(index, round)));
                }
                for index in first..end {
                    assert_eq!(
                        view.get(key(index)),
                        if index % 2 == 0 {
                            None
                        } else {
                            Some(value(index, round))
                        }
                    );
                }
                for index in (first..end).filter(|index| index % 2 == 0) {
                    assert_eq!(view.insert(key(index), value(index, round)), Ok(None));
                }
            })
            .unwrap();
        }
        assert_eq!(map.len(), Ok(RECORDS as usize));
        assert_eq!(map.allocated_bytes(), Ok(allocated));
    }
    report("mixed", RECORDS * 4 * ROUNDS, start);

    let start = Instant::now();
    let (slots, count) = map.release().unwrap();
    let pointer = slots.as_ptr();
    let mut reopened =
        Map::admit(slots, count, RECORDS as usize).unwrap_or_else(|_| panic!("native admission"));
    report("native_admission_scan", RECORDS, start);
    reopened
        .with(|view| {
            for index in 0..RECORDS {
                assert_eq!(view.get(key(index)), Some(value(index, ROUNDS)));
            }
        })
        .unwrap();
    let (slots, count) = reopened.release().unwrap();
    assert_eq!(slots.as_ptr(), pointer);
    let map =
        Map::admit(slots, count, RECORDS as usize).unwrap_or_else(|_| panic!("native admission"));

    let start = Instant::now();
    let candidate = map
        .rebuild_in(RECORDS as usize, std::alloc::Global)
        .unwrap();
    report("native_rebuild", RECORDS, start);
    let (old_slots, old_count) = map.release().unwrap();
    let (new_slots, new_count) = candidate.release().unwrap();
    assert_eq!(new_count, old_count);
    assert_ne!(new_slots.as_ptr(), old_slots.as_ptr());
    let map = Map::admit(new_slots, new_count, RECORDS as usize)
        .unwrap_or_else(|_| panic!("rebuilt admission"));
    drop(old_slots);

    let start = Instant::now();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/evidence/native-map-firehose");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("checkpoint-{}.bin", std::process::id()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let mut output = BufWriter::new(file);
    map.export(7, |chunk| output.write_all(chunk)).unwrap();
    output.flush().unwrap();
    let length = output.get_ref().metadata().unwrap().len() as usize;
    drop(output);
    report("streamed_checkpoint_file", RECORDS, start);
    assert_eq!(length, 56 + RECORDS as usize * 16);
    drop(map);
    let start = Instant::now();
    let mut input = File::open(&path).unwrap();
    let mut largest_read = 0;
    let mut recovered = Map::from_reader(length, 7, RECORDS as usize, |buffer| {
        largest_read = largest_read.max(buffer.len());
        input.read_exact(buffer)
    })
    .unwrap();
    assert_eq!(largest_read, 4096);
    assert_eq!(input.read(&mut [0]).unwrap(), 0);
    report("streamed_portable_recovery_file", RECORDS, start);
    recovered
        .with(|mut view| {
            for index in 0..RECORDS {
                assert_eq!(view.get(key(index)), Some(value(index, ROUNDS)));
            }
            assert_eq!(view.remove(key(0)), Some(value(0, ROUNDS)));
            assert_eq!(view.insert(key(RECORDS), 99), Ok(None));
            assert_eq!(view.get(key(RECORDS)), Some(99));
        })
        .unwrap();
    println!(
        "verified=true checkpoint_bytes={length} max_read_bytes={largest_read} rounds={ROUNDS} batch={BATCH}"
    );
}
