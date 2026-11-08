//! encoding contracts.
use super::*;

#[test]
fn record_budget_identity_and_configuration_refusals_preserve_retained_bytes() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(memory)).unwrap());
    let limits = Limits {
        max_entries: NonZeroUsize::new(32).unwrap(),
        max_message_bytes: 4096,
        ..limits()
    };
    let mut log = Log::new(limits);
    store.save(&key(), identity(), &log).unwrap();
    let bytes = store.map.get(&key().0);
    writes.reset();
    let wrong = LogIdentity { epoch: 8 };
    assert!(matches!(
        store.load(&key(), wrong, limits),
        Err(StoreError::IdentityMismatch)
    ));
    assert_eq!(
        store.save(&key(), wrong, &log),
        Err(StoreError::IdentityMismatch)
    );
    let other = Limits {
        max_message_bytes: 1,
        ..limits
    };
    assert!(matches!(
        store.load(&key(), identity(), other),
        Err(StoreError::LimitsMismatch)
    ));
    // Fill the text ceiling exactly; encoded row metadata exceeds the record ceiling.
    for time in 0..16 {
        log.append(input(time, &"x".repeat(4095))).unwrap();
    }
    assert_eq!(
        store.save(&key(), identity(), &log),
        Err(StoreError::RecordTooLarge)
    );
    let unsupported = Limits {
        max_entries: NonZeroUsize::new(MAX_ENTRIES + 1).unwrap(),
        ..limits
    };
    assert!(matches!(
        store.load(&key(), identity(), unsupported),
        Err(StoreError::UnsupportedLimits)
    ));
    assert_eq!(
        store.save(&key(), identity(), &Log::new(unsupported)),
        Err(StoreError::UnsupportedLimits)
    );
    assert_eq!(store.map.get(&key().0), bytes);
    assert_eq!(writes.counts().writes, 0);
}

#[test]
fn malformed_bytes_are_never_empty_or_reset_fallbacks() {
    let mut log = Log::new(limits());
    log.append(input(10, "utf8-message")).unwrap();
    let valid = codec::encode_snapshot(identity(), &log.snapshot()).unwrap();
    assert_eq!(
        valid,
        b"ICL\x03\x07\x04\x40\x10\x80\x80\x04\x01\x01\x01\x0a\x01\x00\x0a\x01\x00\x01w\x00\x0cutf8-message"
    );
    for end in 0..valid.len() {
        assert!(codec::decode(&valid[..end], identity(), limits()).is_err());
    }
    let mut bytes = valid.clone();
    bytes.push(0);
    assert!(matches!(
        codec::decode(&bytes, identity(), limits()),
        Err(StoreError::MalformedRecord)
    ));
    let mut bytes = valid.clone();
    bytes[3] = 255;
    assert!(matches!(
        codec::decode(&bytes, identity(), limits()),
        Err(StoreError::UnsupportedFormat)
    ));
    let message = valid
        .windows(b"utf8-message".len())
        .position(|bytes| bytes == b"utf8-message")
        .unwrap();
    let mut bytes = valid;
    bytes[message] = 255;
    assert!(matches!(
        codec::decode(&bytes, identity(), limits()),
        Err(StoreError::MalformedRecord)
    ));

    let mut state = log.snapshot();
    state.entries[0].sequence = 1;
    let bytes = codec::encode_snapshot(identity(), &state).unwrap();
    assert!(matches!(
        codec::decode(&bytes, identity(), limits()),
        Err(StoreError::InvalidState(RestoreError::InvalidSequence))
    ));
    state = log.snapshot();
    state.entries[0].truncated = true;
    let bytes = codec::encode_snapshot(identity(), &state).unwrap();
    let mut store = store();
    store.map.insert(key().0, bytes.clone());
    assert_eq!(
        store.save(&key(), identity(), &log),
        Err(StoreError::InvalidState(RestoreError::InvalidEntry))
    );
    assert_eq!(store.map.get(&key().0), Some(bytes));
    assert!(matches!(
        codec::decode(&vec![0; MAX_RECORD_BYTES + 1], identity(), limits()),
        Err(StoreError::RecordTooLarge)
    ));
}

#[test]
fn wire_checkpoint_admits_configured_fields_before_owned_restoration() {
    let mut log = Log::new(limits());
    log.append(input(10, "event")).unwrap();
    for field in 0..3 {
        let mut state = log.snapshot();
        match field {
            0 => state.entries[0].source = "s".repeat(limits().max_label_bytes + 1),
            1 => state.entries[0].topic = Some("t".repeat(limits().max_label_bytes + 1)),
            _ => state.entries[0].message = "m".repeat(limits().max_message_bytes + 1),
        }
        let bytes = codec::encode_snapshot(identity(), &state).unwrap();
        assert!(matches!(
            codec::decode(&bytes, identity(), limits()),
            Err(StoreError::MalformedRecord)
        ));
    }
    let mut state = log.snapshot();
    state.entries = vec![state.entries[0].clone(); limits().max_entries.get() + 1];
    let bytes = codec::encode_snapshot(identity(), &state).unwrap();
    assert!(matches!(
        codec::decode(&bytes, identity(), limits()),
        Err(StoreError::InvalidState(RestoreError::TooManyEntries))
    ));
}

#[test]
fn wire_budgets_admit_maximum_rows_and_large_valid_checkpoints() {
    for (count, message_bytes) in [(MAX_ENTRIES, 0), (14, MAX_MESSAGE_BYTES)] {
        let limits = Limits {
            max_entries: NonZeroUsize::new(MAX_ENTRIES).unwrap(),
            max_message_bytes: MAX_MESSAGE_BYTES,
            max_label_bytes: MAX_LABEL_BYTES,
            max_text_bytes: MAX_TEXT_BYTES,
        };
        let message = "m".repeat(message_bytes);
        let label = "s".repeat(if count == MAX_ENTRIES {
            1
        } else {
            MAX_LABEL_BYTES
        });
        let mut log = Log::new(limits);
        for _ in 0..count {
            log.append(EntryInput {
                created_at_secs: u64::MAX,
                level: Level::Error,
                source: &label,
                topic: Some(&label),
                message: &message,
            })
            .unwrap();
        }
        let bytes = codec::encode_snapshot(identity(), &log.snapshot()).unwrap_or_else(|error| {
            panic!("{count} rows with {message_bytes} message bytes: {error}")
        });
        assert!(bytes.len() <= MAX_RECORD_BYTES);
        assert_eq!(
            codec::decode(&bytes, identity(), limits)
                .unwrap()
                .snapshot(),
            log.snapshot()
        );
    }
}

#[test]
fn text_budget_eviction_and_forged_retained_usage_are_admitted_before_writes() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(memory)).unwrap());
    let bounded = Limits {
        max_text_bytes: 8,
        ..limits()
    };
    let mut log = Log::new(bounded);
    for time in 0..3 {
        log.append(input(time, "a")).unwrap();
    }
    store.save(&key(), identity(), &log).unwrap();
    let receipt = log.append(input(3, "12345")).unwrap();
    assert_eq!(receipt.sequence, 3);
    assert_eq!(receipt.evicted, 2);
    assert!(!receipt.truncated);
    assert_eq!(log.retained_text_bytes(), 8);
    store.save(&key(), identity(), &log).unwrap();
    let state = log.snapshot();
    let bytes = store.map.get(&key().0).unwrap();
    assert!(matches!(
        log.append(input(4, "12345678")),
        Err(AppendError::TextBudgetExceeded { .. })
    ));
    assert_eq!(log.snapshot(), state);
    writes.reset();
    store.save(&key(), identity(), &log).unwrap();
    assert_eq!(writes.counts().writes, 0);
    assert_eq!(store.map.get(&key().0), Some(bytes));
    let restored = store.load(&key(), identity(), bounded).unwrap().unwrap();
    assert_eq!(restored.retained_text_bytes(), 8);
    assert_eq!(restored.snapshot(), state);

    let mut forged = state;
    forged.limits.max_text_bytes = 7;
    let forged_bytes = codec::encode_snapshot(identity(), &forged).unwrap();
    store.map.insert(key().0, forged_bytes.clone());
    writes.reset();
    assert!(matches!(
        store.load(&key(), identity(), forged.limits),
        Err(StoreError::InvalidState(RestoreError::TextBudgetExceeded))
    ));
    assert_eq!(
        store.save(&key(), identity(), &Log::new(forged.limits)),
        Err(StoreError::InvalidState(RestoreError::TextBudgetExceeded))
    );
    assert_eq!(
        store.remove(&key(), identity(), forged.limits),
        Err(StoreError::InvalidState(RestoreError::TextBudgetExceeded))
    );
    assert_eq!(writes.counts().writes, 0);
    assert_eq!(store.map.get(&key().0), Some(forged_bytes));
    assert!(matches!(
        store.load(
            &key(),
            identity(),
            Limits {
                max_text_bytes: MAX_TEXT_BYTES + 1,
                ..limits()
            }
        ),
        Err(StoreError::UnsupportedLimits)
    ));
}

#[test]
fn retired_checkpoint_requires_explicit_host_row_administration() {
    let memory = MeteredMemory::<VectorMemory>::default();
    let writes = memory.clone();
    let mut store = LoggingStore::new(StableBTreeMap::create(admitted(memory)).unwrap());
    let retired =
        b"ICL\x02\x07\x04\x40\x10\x01\x01\x01\x0a\x01\x00\x0a\x01\x00\x01w\x00\x0cutf8-message"
            .to_vec();
    store.map.insert(key().0, retired.clone());
    writes.reset();
    assert!(matches!(
        store.load(&key(), identity(), limits()),
        Err(StoreError::UnsupportedFormat)
    ));
    assert_eq!(
        store.save(&key(), identity(), &Log::new(limits())),
        Err(StoreError::UnsupportedFormat)
    );
    assert_eq!(
        store.remove(&key(), identity(), limits()),
        Err(StoreError::UnsupportedFormat)
    );
    assert_eq!(store.map.get(&key().0), Some(retired));
    assert_eq!(writes.counts().writes, 0);
    // The host deliberately retires this row; neither loading nor saving resets it.
    store.map.remove(&key().0);
    store.save(&key(), identity(), &Log::new(limits())).unwrap();
    assert!(
        store
            .load(&key(), identity(), limits())
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn later_field_refusal_precedes_an_earlier_total_text_excess() {
    let mut log = Log::new(limits());
    log.append(input(1, "a")).unwrap();
    log.append(input(2, "b")).unwrap();
    let mut forged = log.snapshot();
    forged.limits.max_text_bytes = 0;
    forged.entries[1].message = "x".repeat(forged.limits.max_message_bytes + 1);
    let bytes = codec::encode_snapshot(identity(), &forged).unwrap();
    assert!(matches!(
        codec::decode(&bytes, identity(), forged.limits),
        Err(StoreError::MalformedRecord)
    ));
}
