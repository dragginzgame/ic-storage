use super::*;
use alloc::vec;
use sha2::{Digest, Sha256};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn fresh() -> State {
    State::new_in(Schema::Wide, Global)
}

#[test]
fn owner_revision_retries_capacity_and_reuse_preserve_both_indexes() {
    let mut state = fresh();
    let name = "é".repeat(64); // 128 bytes, not 128 characters.
    for id in 1..=16 {
        let owner = [id as u8 + 10; 29];
        let created = state.put(&owner, id, 0, &name).unwrap();
        assert_eq!(created.revision, 1);
        let unchanged = state.checkpoint(7).unwrap();
        assert_eq!(state.put(&owner, id, 0, &name), Ok(created));
        assert_eq!(state.checkpoint(7).unwrap(), unchanged);
        assert_eq!(state.get(&owner, id), Ok(created));
    }
    let snapshot = state.checkpoint(7).unwrap();
    for (owner, id, revision, name, error) in [
        (&[4][..], 1, 1, "new", Error::Unauthenticated),
        (&[11; 29][..], 17, 0, "new", Error::Registered),
        (&[99; 29][..], 1, 1, "new", Error::Forbidden),
        (&[11; 29][..], 1, 0, "different retry", Error::Revision),
        (&[11; 29][..], 1, u64::MAX, "new", Error::Revision),
        (&[99; 29][..], 17, 0, "new", Error::Capacity),
        (&[99; 30][..], 17, 0, "new", Error::Invalid),
        (&[11; 29][..], 1, 1, " \t\n", Error::Invalid),
        (&[11; 29][..], 0, 1, "new", Error::Invalid),
    ] {
        assert_eq!(state.put(owner, id, revision, name), Err(error));
        assert_eq!(state.checkpoint(7).unwrap(), snapshot);
    }
    assert_eq!(
        state.put(&[11; 29], 1, 1, &"a".repeat(129)),
        Err(Error::Invalid)
    );
    let before = state.usage().unwrap().map(|usage| usage.allocated_bytes);
    for cycle in 0..32 {
        for id in 1..=16 {
            let owner = [id as u8 + 10; 29];
            state.remove(&owner, id, 1).unwrap();
            assert_eq!(state.get(&owner, id), Err(Error::Missing));
            // Both the row identity and owner index must have been reclaimed.
            state.put(&owner, id + 100, 0, &name).unwrap();
            state.remove(&owner, id + 100, 1).unwrap();
            state.put(&owner, id, 0, &name).unwrap();
        }
        state.validate().unwrap();
        assert_eq!(
            state.usage().unwrap().map(|usage| usage.allocated_bytes),
            before,
            "cycle {cycle}"
        );
    }
    let changed = state.put(&[11; 29], 1, 1, "updated").unwrap();
    assert_eq!(changed.revision, 2);
    assert_eq!(state.put(&[11; 29], 1, 1, "updated"), Ok(changed));
    assert_eq!(state.get(&[99], 1), Err(Error::Forbidden));
    let usage = state.usage().unwrap();
    println!(
        "profile roots=2 records=16 native_allocated_bytes={} checkpoint_bytes={}",
        usage.iter().map(|v| v.allocated_bytes).sum::<usize>(),
        state.checkpoint(7).unwrap().len()
    );
    assert!(usage.iter().map(|v| v.allocated_bytes).sum::<usize>() < 32 * 65536);
}

#[test]
fn compatible_live_readmission_and_destroyed_source_portable_conversion() {
    let mut state = fresh();
    for id in 1..=16 {
        state.put(&[id as u8 + 10], id, 0, "original").unwrap();
    }
    state.put(&[11], 1, 1, "second revision").unwrap();
    let checkpoint = state.checkpoint(91).unwrap();
    let (rows, owners) = state.release().unwrap();
    // These are real live allocations, not a raw retained-pointer admission claim.
    let rows = Map::admit(rows.release().unwrap(), LIMIT).unwrap_or_else(|_| panic!("rows"));
    let owners = Map::admit(owners.release().unwrap(), LIMIT).unwrap_or_else(|_| panic!("owners"));
    let state = State::admit(rows, owners, Schema::Wide).unwrap();
    assert_eq!(state.checkpoint(91).unwrap(), checkpoint);
    drop(state);
    let mut target = State::recover(&checkpoint, 91, Schema::Wide, Schema::Narrow, Global).unwrap();
    assert_eq!(target.get(&[11], 1).unwrap().revision, 2);
    assert_eq!(
        target.put(&[11], 1, 1, "second revision").unwrap().revision,
        2
    );
    assert_eq!(
        target.put(&[11], 1, 2, "after recovery").unwrap().revision,
        3
    );
    target.remove(&[12], 2, 1).unwrap();
    target.put(&[12], 20, 0, "reused owner").unwrap();
    let converted = target.checkpoint(92).unwrap();
    drop(target);
    let target = State::recover(&converted, 92, Schema::Narrow, Schema::Narrow, Global).unwrap();
    assert_eq!(
        target.get(&[12], 20).unwrap().name.as_slice(),
        b"reused owner"
    );
    assert!(matches!(
        State::recover(&checkpoint, 91, Schema::Narrow, Schema::Narrow, Global),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        State::recover(&converted, 92, Schema::Narrow, Schema::Wide, Global),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        State::recover(&checkpoint, 90, Schema::Wide, Schema::Wide, Global),
        Err(Error::Invalid)
    ));
    // A valid source revision outside the target width refuses explicit conversion.
    let mut wide = fresh();
    let value = Profile::new(1, u64::from(u32::MAX) + 1, &[11], "large revision").unwrap();
    wide.grant
        .update(|mut v| {
            v.insert(
                ROWS,
                &1u64.to_be_bytes(),
                value.encode(Schema::Wide).unwrap().as_slice(),
            )
        })
        .unwrap()
        .unwrap();
    wide.grant
        .update(|mut v| v.insert(OWNERS, &[11], &1u64.to_le_bytes()))
        .unwrap()
        .unwrap();
    wide.validate().unwrap();
    let source = wide.checkpoint(5).unwrap();
    assert!(matches!(
        State::recover(&source, 5, Schema::Wide, Schema::Narrow, Global),
        Err(Error::Conversion)
    ));
    assert_eq!(wide.checkpoint(5).unwrap(), source);
}

#[test]
fn invalid_semantics_and_mixed_root_snapshots_refuse_without_repair() {
    let mut state = fresh();
    state.put(&[11], 1, 0, "valid").unwrap();
    let snapshot = state.checkpoint(3).unwrap();
    for index in 0..snapshot.len() {
        let mut bad = snapshot.clone();
        bad[index] ^= 1;
        assert!(
            State::recover(&bad, 3, Schema::Wide, Schema::Wide, Global).is_err(),
            "byte {index}"
        );
    }
    for end in 0..snapshot.len() {
        assert!(State::recover(&snapshot[..end], 3, Schema::Wide, Schema::Wide, Global).is_err());
    }
    let mut trailing = snapshot.clone();
    trailing.push(0);
    assert!(State::recover(&trailing, 3, Schema::Wide, Schema::Wide, Global).is_err());
    // Correct outer seals cannot authorize different schemas, root roles, epochs
    // or impossible section lengths. Exercise parsing beyond checksum refusal.
    for (at, value) in [
        (8, 99u64),
        (16, 4),
        (24, 2),
        (32, 1),
        (40, u64::MAX),
        (48, u64::MAX),
    ] {
        let mut bad = snapshot[..snapshot.len() - 32].to_vec();
        bad[at..at + 8].copy_from_slice(&value.to_le_bytes());
        let seal = Sha256::digest(&bad);
        bad.extend_from_slice(&seal);
        assert!(matches!(
            State::recover(&bad, 3, Schema::Wide, Schema::Wide, Global),
            Err(Error::Invalid)
        ));
    }
    for bad_profile in [
        Profile::new(2, 1, &[11], "wrong row identity")
            .unwrap()
            .encode(Schema::Wide)
            .unwrap(),
        Profile::new(1, 1, &[11], "wrong row schema")
            .unwrap()
            .encode(Schema::Narrow)
            .unwrap(),
    ] {
        let mut candidate =
            State::recover(&snapshot, 3, Schema::Wide, Schema::Wide, Global).unwrap();
        candidate
            .grant
            .update(|mut rows| rows.insert(ROWS, &1u64.to_be_bytes(), bad_profile.as_slice()))
            .unwrap()
            .unwrap();
        let bad = candidate.checkpoint(3).unwrap();
        assert!(matches!(
            State::recover(&bad, 3, Schema::Wide, Schema::Wide, Global),
            Err(Error::Invalid)
        ));
    }
    // Correctly sealed but inconsistent application data must still refuse.
    for bad_value in [2u64.to_le_bytes().to_vec(), vec![1]] {
        state
            .grant
            .update(|mut v| v.insert(OWNERS, &[11], &bad_value))
            .unwrap()
            .unwrap();
        let bad = state.checkpoint(3).unwrap();
        assert!(matches!(
            State::recover(&bad, 3, Schema::Wide, Schema::Wide, Global),
            Err(Error::Invalid)
        ));
    }
    let mut second = fresh();
    second.put(&[12], 2, 0, "different family").unwrap();
    let second = second.checkpoint(3).unwrap();
    let row_len = u64::from_le_bytes(snapshot[40..48].try_into().unwrap()) as usize;
    let second_rows = u64::from_le_bytes(second[40..48].try_into().unwrap()) as usize;
    let mut mixed = snapshot[..56 + row_len].to_vec();
    let other_owners = &second[56 + second_rows..second.len() - 32];
    mixed[48..56].copy_from_slice(&(other_owners.len() as u64).to_le_bytes());
    mixed.extend_from_slice(other_owners);
    let seal = Sha256::digest(&mixed);
    mixed.extend_from_slice(&seal);
    assert!(matches!(
        State::recover(&mixed, 3, Schema::Wide, Schema::Wide, Global),
        Err(Error::Invalid)
    ));
}

#[test]
fn codec_is_bounded_exact_and_does_not_admit_private_rust_layouts() {
    let value = Profile::new(1, 2, &[], "x").unwrap();
    let bytes = value.encode(Schema::Wide).unwrap();
    let mut expected = b"PROFILE1\x01".to_vec();
    expected.extend_from_slice(&1u64.to_le_bytes());
    expected.extend_from_slice(&2u64.to_le_bytes());
    expected.extend_from_slice(&[0, 1, 0, b'x']);
    assert_eq!(bytes.as_slice(), expected);
    assert_eq!(Profile::decode(&expected, Schema::Wide), Ok(value));
    for end in 0..expected.len() {
        assert!(Profile::decode(&expected[..end], Schema::Wide).is_err());
    }
    for (at, byte) in [
        (8, 9),
        (9, 0),
        (17, 0),
        (25, 30),
        (26, 129),
        (28, 0xff),
        (28, b' '),
    ] {
        let mut bad = expected.clone();
        bad[at] = byte;
        assert!(Profile::decode(&bad, Schema::Wide).is_err(), "byte {at}");
    }
    expected.push(0);
    assert!(Profile::decode(&expected, Schema::Wide).is_err());
    assert_eq!(
        Profile::new(1, 1, &[4], "name"),
        Err(Error::Unauthenticated)
    );
    for schema in [Schema::Wide, Schema::Narrow] {
        let full =
            Profile::new(u64::MAX, u64::from(u32::MAX), &[255; 29], &"é".repeat(64)).unwrap();
        let bytes = full.encode(schema).unwrap();
        assert_eq!(
            bytes.as_slice().len(),
            if schema == Schema::Wide { 185 } else { 181 }
        );
        assert_eq!(Profile::decode(bytes.as_slice(), schema), Ok(full));
    }
}

#[test]
fn caught_failure_between_root_writes_quarantines_the_whole_family() {
    let mut state = fresh();
    state.put(&[11], 1, 0, "before").unwrap();
    let committed = state.checkpoint(1).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = state.put_after_row(&[12], 2, 0, "unfinished", || panic!("between roots"));
        }))
        .is_err()
    );
    assert_eq!(state.get(&[11], 1), Err(Error::Poisoned));
    assert_eq!(state.put(&[11], 1, 1, "later"), Err(Error::Poisoned));
    assert_eq!(state.checkpoint(1), Err(Error::Poisoned));
    assert!(matches!(state.release(), Err(Error::Poisoned)));
    let mut replacement =
        State::recover(&committed, 1, Schema::Wide, Schema::Wide, Global).unwrap();
    assert_eq!(replacement.get(&[12], 2), Err(Error::Missing));
    replacement.put(&[12], 2, 0, "explicit recovery").unwrap();
}
