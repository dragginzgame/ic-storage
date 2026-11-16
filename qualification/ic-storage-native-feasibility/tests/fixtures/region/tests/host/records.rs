use super::*;

#[test]
fn narrow_checkpoint_reconstruction_preserves_schema_and_vector_on_failed_replacement() {
    let record = Record::V3(record::narrow::V3::new(u32::MAX, u64::MAX).unwrap());
    for count in [0, 3, WORDS] {
        let saved = checkpoint::encode_record(record, &vec![17; count], 9).unwrap();
        let mut host = populated();
        let old = host.export(7).unwrap();
        assert_eq!(
            host.recover_with(&saved, 9, ImportFault::BeforePublish),
            Err(Failure::Injected)
        );
        assert_eq!(host.export(7).unwrap(), old);
        host.recover(&saved, 9).unwrap();
        assert_eq!(host.record, record);
        assert_eq!(host.export(9).unwrap(), saved);
        // V2-only access cannot silently treat a V3 payload as its predecessor.
        assert_eq!(
            host.update_record(|_| panic!("wrong schema callback")),
            Err(Failure::RecordVersion)
        );
        host.update(|mut view| {
            view.clear();
            view.shrink_to_fit();
            view.push(55).unwrap();
        })
        .unwrap();
        assert_eq!(host.record, record);
        assert_eq!(
            checkpoint::decode_record(&host.export(9).unwrap(), 9)
                .unwrap()
                .values,
            [55]
        );
    }
}

#[test]
fn scoped_record_updates_keep_prior_writes_on_business_error_and_finalize_forgotten_views() {
    let mut host = populated();
    assert_eq!(
        host.update_record(|_| panic!("V1 callback")),
        Err(Failure::RecordVersion)
    );
    assert!(!host.poisoned);
    host.convert(43, false).unwrap();
    let root = host.root;
    let backing = host.backing;
    let result = host.update_record(|mut view| {
        assert_eq!((view.counter(), view.ceiling()), (41, 43));
        view.add(1).unwrap();
        view.add(2)
    });
    assert_eq!(result, Ok(Err(record::Error::Ceiling)));
    assert!(!host.poisoned);
    assert_eq!(host.update_record(|view| view.counter()), Ok(42));
    host.update_record(|mut view| {
        view.add(1).unwrap();
        #[expect(
            clippy::forget_non_drop,
            reason = "view destruction cannot own host finalization"
        )]
        mem::forget(view);
    })
    .unwrap();
    assert!(!host.poisoned);
    assert_eq!(host.root, root);
    assert_eq!(host.backing, backing);
    let saved = host.export(7).unwrap();
    let decoded = checkpoint::decode_record(&saved, 7).unwrap();
    assert_eq!(decoded.record, Record::V2(record::V2::new(43, 43).unwrap()));
    let mut fresh = ProviderHost::new();
    fresh.recover(&saved, 7).unwrap();
    assert_eq!(
        fresh.update_record(|mut view| view.add(1)),
        Ok(Err(record::Error::Ceiling))
    );
    assert_eq!(
        fresh.update_record(|view| (view.counter(), view.ceiling())),
        Ok((43, 43))
    );
    fresh.update(|mut view| view.push(99)).unwrap().unwrap();
}

#[test]
fn record_unwind_poison_blocks_all_access_until_explicit_checkpoint_replacement() {
    for forget in [false, true] {
        let mut host = populated();
        host.convert(100, false).unwrap();
        let saved = host.export(7).unwrap();
        let backing = host.backing;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = host.update_record(|mut view| {
                view.add(1).unwrap();
                if forget {
                    #[expect(
                        clippy::forget_non_drop,
                        reason = "forgotten views must not evade poison"
                    )]
                    mem::forget(view);
                }
                panic!("record mutation panic");
            });
        }));
        assert!(result.is_err());
        assert_eq!(host.record.counter(), 42, "host unwinding is not rollback");
        assert_eq!(host.backing, backing);
        assert_eq!(
            host.update_record(|_| panic!("poisoned callback")),
            Err(Failure::Poisoned)
        );
        assert_eq!(
            host.update(|_| panic!("poisoned vector")),
            Err(Failure::Poisoned)
        );
        assert_eq!(host.export(7), Err(Failure::Poisoned));
        assert_eq!(
            host.recover_with(&saved, 7, ImportFault::BeforePublish),
            Err(Failure::Injected)
        );
        assert!(host.poisoned);
        assert_eq!(host.record.counter(), 42);
        host.recover(&saved, 7).unwrap();
        assert_eq!(host.update_record(|mut view| view.add(2)), Ok(Ok(())));
        assert_eq!(host.update_record(|view| view.counter()), Ok(43));
        assert_eq!(
            host.backing[..host.root.length],
            backing[..host.root.length]
        );
    }
}

#[test]
fn scalar_conversion_preserves_the_vector_and_refuses_before_publication() {
    for (count, remove) in [
        (0, false),
        (3, false),
        (3, true),
        (WORDS, false),
        (WORDS, true),
    ] {
        let (saved, expected) = {
            let mut host = ProviderHost::new();
            host.update(|mut view| {
                for i in 0..count {
                    view.push(i as u64).unwrap();
                }
                if remove {
                    view.remove_first();
                }
            })
            .unwrap();
            let original_record = host.record;
            let root = host.root;
            let backing = host.backing;
            let pointer = host.backing.as_ptr();
            for (ceiling, fault, error) in [
                (40, false, Failure::Record(record::Error::Ceiling)),
                (100, true, Failure::Injected),
            ] {
                assert_eq!(host.convert(ceiling, fault), Err(error));
                assert_eq!(host.record, original_record);
                assert_eq!(host.root, root);
                assert_eq!(host.backing, backing);
                assert!(!host.poisoned);
            }
            host.convert(100, false).unwrap();
            assert_eq!(host.root, root);
            assert_eq!(host.backing, backing);
            assert_eq!(host.backing.as_ptr(), pointer);
            assert_eq!(host.convert(200, false), Err(Failure::RecordVersion));
            host.update_record(|mut view| view.add(1)).unwrap().unwrap();
            host.update(|mut view| {
                if view.values().len() == WORDS {
                    assert_eq!(view.push(99), Err(Refusal::Limit));
                } else {
                    view.push(99).unwrap();
                }
            })
            .unwrap();
            let saved = host.export(7).unwrap();
            assert_eq!(
                checkpoint::decode(&saved, 7),
                Err(checkpoint::Error::Format)
            );
            let expected = checkpoint::decode_record(&saved, 7).unwrap();
            assert_eq!(
                expected.record,
                Record::V2(record::V2::new(42, 100).unwrap())
            );

            (saved, expected)
        };
        // The source host and every loan have ended; reconstruct independently.
        let mut restored = ProviderHost::new();
        restored.recover(&saved, 7).unwrap();
        assert_eq!(restored.record, expected.record);
        assert_eq!(restored.root.length, expected.values.len());
        assert_eq!(restored.root.capacity, expected.values.len());
        restored
            .update(|mut view| {
                assert_eq!(view.values(), expected.values);
                view.clear();
                view.shrink_to_fit();
                view.push(55).unwrap();
            })
            .unwrap();
        assert_eq!(restored.record, expected.record);
    }
}

#[test]
fn v2_recovery_failure_and_poison_do_not_admit_conversion_or_replace_state() {
    let mut host = populated();
    host.convert(100, false).unwrap();
    let saved = host.export(7).unwrap();
    let record = host.record;
    let root = host.root;
    let backing = host.backing;
    for fault in [ImportFault::AfterValue(1), ImportFault::BeforePublish] {
        assert_eq!(host.recover_with(&saved, 7, fault), Err(Failure::Injected));
        assert_eq!(host.record, record);
        assert_eq!(host.root, root);
        assert_eq!(host.backing, backing);
        assert!(!host.poisoned);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        host.update(|mut view| {
            view.values_mut()[0] = 999;
            panic!("injected");
        })
        .unwrap();
    }));
    assert!(result.is_err());
    assert_eq!(host.convert(200, false), Err(Failure::Poisoned));
    assert_eq!(host.export(7), Err(Failure::Poisoned));
    host.recover(&saved, 7).unwrap();
    assert_eq!(host.record, record);
    assert_eq!(host.root, root);
    assert_eq!(host.backing[..root.length], backing[..root.length]);
    assert!(!host.poisoned);
}
