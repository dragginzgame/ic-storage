//! continuity contracts.
use super::*;

#[test]
fn empty_and_exhausted_checkpoints_preserve_watermarks_across_reopen() {
    let backing = VectorMemory::default();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(backing.clone())).unwrap());
    let mut log = Log::new(limits());
    log.append(input(20, "expired")).unwrap();
    log.prune_expired(21, 0, 1);
    store.save(&key(), identity(), &log).unwrap();
    drop(store);
    let mut store = LoggingStore::new(StableBTreeMap::load(admitted(backing)).unwrap());
    let mut log = store.load(&key(), identity(), limits()).unwrap().unwrap();
    assert!(log.is_empty());
    assert_eq!(
        log.append(input(19, "bad")),
        Err(AppendError::TimestampRegressed {
            previous: 20,
            current: 19
        })
    );
    assert_eq!(
        log.append(input(20, "next"))
            .map(|receipt| receipt.sequence),
        Ok(1)
    );
    let exhausted = Log::restore(LogSnapshot {
        limits: limits(),
        entries: vec![],
        next_sequence: None,
        last_created_at_secs: Some(20),
    })
    .unwrap();
    store.save(&key(), identity(), &exhausted).unwrap();
    let mut restored = store.load(&key(), identity(), limits()).unwrap().unwrap();
    assert_eq!(
        restored.append(input(21, "bad")),
        Err(AppendError::SequenceExhausted)
    );
    assert_eq!(
        store.save(&key(), identity(), &log),
        Err(StoreError::StateRegression)
    );
}

#[test]
fn identical_and_refused_checkpoints_do_not_write_or_rewrite_issued_history() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(memory)).unwrap());
    let mut log = Log::new(limits());
    for time in 10..13 {
        log.append(input(time, "event")).unwrap();
    }
    store.save(&key(), identity(), &log).unwrap();
    let bytes = store.map.get(&key().0);
    writes.reset();
    store.save(&key(), identity(), &log).unwrap();
    assert_eq!(writes.counts().writes, 0);
    for mode in 0..3 {
        let mut state = log.snapshot();
        match mode {
            0 => {
                state.entries.clear();
                state.next_sequence = Some(2);
            }
            1 => state.entries[0].message = "rewritten".into(),
            _ => {
                state.entries.clear();
                state.last_created_at_secs = Some(13);
            }
        }
        let invalid = Log::restore(state).unwrap();
        assert_eq!(
            store.save(&key(), identity(), &invalid),
            Err(StoreError::StateRegression)
        );
        assert_eq!(writes.counts().writes, 0);
        assert_eq!(store.map.get(&key().0), bytes);
    }
    let original = log.snapshot();
    log.prune_expired(13, 0, 4);
    store.save(&key(), identity(), &log).unwrap();
    writes.reset();
    let resurrected = Log::restore(original).unwrap();
    assert_eq!(
        store.save(&key(), identity(), &resurrected),
        Err(StoreError::StateRegression)
    );
    assert_eq!(writes.counts().writes, 0);
    assert!(
        store
            .load(&key(), identity(), limits())
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rolling_retention_preserves_overlap_and_refuses_rewrites_without_writes() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(memory)).unwrap());
    let mut log = Log::new(limits());
    for time in 0..4 {
        log.append(input(time, "original")).unwrap();
    }
    store.save(&key(), identity(), &log).unwrap();
    for time in 4..6 {
        log.append(input(time, "continued")).unwrap();
    }
    store.save(&key(), identity(), &log).unwrap();
    let before = store.map.get(&key().0);
    let mut rewritten = log.snapshot();
    rewritten.entries[0].message = "rewritten overlap".into();
    let rewritten = Log::restore(rewritten).unwrap();
    writes.reset();
    assert_eq!(
        store.save(&key(), identity(), &rewritten),
        Err(StoreError::StateRegression)
    );
    assert_eq!(writes.counts().writes, 0);
    assert_eq!(store.map.get(&key().0), before);
    // Retention can subsequently remove the entire previously retained window.
    for time in 6..10 {
        log.append(input(time, "new window")).unwrap();
    }
    store.save(&key(), identity(), &log).unwrap();
    assert_eq!(
        store
            .load(&key(), identity(), limits())
            .unwrap()
            .unwrap()
            .snapshot(),
        log.snapshot()
    );
}
