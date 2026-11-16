use super::*;

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
            let Record::V2(value) = &mut host.record else {
                panic!("conversion did not publish V2")
            };
            value.add(1).unwrap();
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
