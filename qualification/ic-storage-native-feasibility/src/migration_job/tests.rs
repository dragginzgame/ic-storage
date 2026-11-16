use super::*;

#[test]
fn independent_initial_envelope_golden() {
    // Separately assembled with Perl pack and Digest::SHA from the wire spec.
    let hex = include_str!("golden.hex").trim();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let job = Job::initial(131072).unwrap();
    assert_eq!(bytes.len(), 104);
    assert_eq!(job.encode().unwrap(), bytes);
    assert_eq!(Job::decode(&bytes).unwrap(), job);
}

fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn sequential_handoff_retry_and_consumption() {
    let initial = Job::initial(131072).unwrap();
    assert_eq!(Job::decode(&initial.encode().unwrap()), Ok(initial));
    let initial = Job::initial(131072).unwrap();
    assert_eq!(
        initial.export(131072, 147456, 2, 41, &[17]),
        Err(Error::Epoch)
    );
    assert_eq!(
        initial.export(147456, 131072, 1, 41, &[17]),
        Err(Error::Domain)
    );
    let mut job = initial.export(131072, 147456, 1, 41, &[17, 23]).unwrap();
    assert_eq!(
        job.export(131072, 147456, 1, 41, &[17, 23]),
        Ok(Job::decode(&job.encode().unwrap()).unwrap())
    );
    assert_eq!(job.export(131072, 147456, 1, 41, &[99]), Err(Error::State));
    let before = job.encode().unwrap();
    assert_eq!(job.consume(131072, 131072, 1), Err(Error::Domain));
    assert_eq!(job.consume(147456, 147456, 1), Err(Error::Domain));
    assert_eq!(job.consume(147456, 131072, 2), Err(Error::Epoch));
    assert_eq!(job.encode().unwrap(), before);
    assert_eq!(job.consume(147456, 131072, 1).unwrap().values, [17, 23]);
    assert_eq!(job.consume(147456, 131072, 1), Err(Error::State));
    assert_eq!(job.export(147456, 131072, 1, 41, &[1]), Err(Error::Epoch));
    let next = job.export(147456, 131072, 2, 41, &[1]).unwrap();
    assert_eq!(Job::decode(&next.encode().unwrap()).unwrap(), next);
}

#[test]
fn bounded_envelope_and_resealed_semantic_refusals() {
    for values in [vec![], vec![7; 512]] {
        let job = Job::initial(131072)
            .unwrap()
            .export(131072, 147456, 1, 41, &values)
            .unwrap();
        let bytes = job.encode().unwrap();
        assert_eq!(Job::decode(&bytes).unwrap(), job);
        for length in 0..bytes.len() {
            assert!(Job::decode(&bytes[..length]).is_err());
        }
        for (offset, value) in [
            (8, 2),
            (10, 4),
            (12, 63),
            (16, 0),
            (24, 1),
            (32, 1),
            (40, 0),
            (44, 1),
            (72, 1),
        ] {
            let mut changed = bytes.clone();
            changed[offset] = value;
            reseal(&mut changed);
            assert!(Job::decode(&changed).is_err(), "offset {offset}");
        }
        let mut changed = bytes;
        changed[24] ^= 1;
        assert_eq!(Job::decode(&changed), Err(Error::Integrity));
    }
    assert_eq!(Job::decode(&vec![0; MAX_BYTES + 1]), Err(Error::Framing));
    let initial = Job::initial(131072).unwrap();
    assert_eq!(
        initial.export(131072, 147456, 1, 41, &[0; 513]),
        Err(Error::Checkpoint(checkpoint::Error::Limit))
    );
    let mut exhausted = initial.export(131072, 147456, 1, 41, &[]).unwrap();
    exhausted.epoch = u64::MAX;
    exhausted.checkpoint = checkpoint::encode_v2(41, &[], u64::MAX).unwrap();
    exhausted.consume(147456, 131072, u64::MAX).unwrap();
    assert_eq!(
        exhausted.export(147456, 131072, 0, 41, &[]),
        Err(Error::Epoch)
    );
}

#[test]
fn cancellation_retires_epoch_keeps_source_and_preserves_retry_state() {
    let mut initial = Job::initial(131072).unwrap();
    assert_eq!(initial.cancel(131072, 147456, 0), Err(Error::State));
    let mut job = initial.export(131072, 147456, 1, 41, &[17, 23]).unwrap();
    let sealed = job.encode().unwrap();
    for (source, target, epoch, error) in [
        (147456, 131072, 1, Error::Domain),
        (131072, 131072, 1, Error::Domain),
        (131072, 147456, 0, Error::Epoch),
        (131072, 147456, 2, Error::Epoch),
    ] {
        assert_eq!(job.cancel(source, target, epoch), Err(error));
        assert_eq!(job.encode().unwrap(), sealed);
    }
    assert_eq!(job.cancel(131072, 147456, 1), Ok(true));
    let cancelled = job.encode().unwrap();
    assert_eq!(cancelled[10..12], 3u16.to_le_bytes());
    assert_eq!(
        &cancelled[16..cancelled.len() - 32],
        &sealed[16..sealed.len() - 32]
    );
    assert_eq!(Job::decode(&cancelled).unwrap(), job);
    assert_eq!(job.cancel(131072, 147456, 1), Ok(false));
    assert_eq!(job.encode().unwrap(), cancelled);
    assert_eq!(job.consume(147456, 131072, 1), Err(Error::State));
    assert_eq!(job.export(131072, 147456, 1, 41, &[99]), Err(Error::Epoch));
    assert_eq!(job.export(147456, 131072, 2, 41, &[99]), Err(Error::Domain));
    let mut next = job.export(131072, 147456, 2, 41, &[99]).unwrap();
    assert_eq!(next.cancel(131072, 147456, 1), Err(Error::Epoch));
    assert_eq!(next.consume(147456, 131072, 2).unwrap().values, [99]);
    assert_eq!(next.cancel(131072, 147456, 2), Err(Error::State));
    let mut last = initial.export(131072, 147456, 1, 41, &[]).unwrap();
    last.epoch = u64::MAX;
    last.checkpoint = checkpoint::encode_v2(41, &[], u64::MAX).unwrap();
    last.cancel(131072, 147456, u64::MAX).unwrap();
    assert_eq!(last.export(131072, 147456, 0, 41, &[]), Err(Error::Epoch));
}

#[test]
fn frozen_admission_is_read_only_and_requires_the_exact_sealed_job() {
    let initial = Job::initial(131072).unwrap();
    assert_eq!(initial.frozen(131072, 147456, 1), Err(Error::State));
    let mut job = initial.export(131072, 147456, 1, 41, &[17, 23]).unwrap();
    let sealed = job.encode().unwrap();
    for _ in 0..2 {
        assert_eq!(job.frozen(131072, 147456, 1).unwrap().values, [17, 23]);
        assert_eq!(job.frozen(147456, 131072, 1), Err(Error::Domain));
        assert_eq!(job.frozen(131072, 131072, 1), Err(Error::Domain));
        assert_eq!(job.frozen(131072, 147456, 2), Err(Error::Epoch));
        assert_eq!(job.encode().unwrap(), sealed);
    }
    job.cancel(131072, 147456, 1).unwrap();
    assert_eq!(job.frozen(131072, 147456, 1), Err(Error::State));
    job = job.export(131072, 147456, 2, 41, &[99]).unwrap();
    job.consume(147456, 131072, 2).unwrap();
    assert_eq!(job.frozen(131072, 147456, 2), Err(Error::State));
}

#[test]
fn staged_progress_roundtrips_and_exact_latest_retry_does_not_advance() {
    for count in [0, 3, 65, 512] {
        let values: Vec<_> = (0..count).collect();
        let mut job = Job::initial(131072)
            .unwrap()
            .export(131072, 147456, 1, 41, &values)
            .unwrap();
        assert_eq!(job.begin_import(147456, 131072, 1).unwrap().values, values);
        assert_eq!(job.cancel(131072, 147456, 1), Err(Error::State));
        assert_eq!(
            job.export(131072, 147456, 2, 41, &values),
            Err(Error::State)
        );
        assert_eq!(job.consume(147456, 131072, 1), Err(Error::State));
        let mut offset = 0;
        loop {
            job = Job::decode(&job.encode().unwrap()).unwrap();
            let (decoded, cursor) = job.importing(147456, 131072, 1).unwrap();
            assert_eq!(decoded.values, values);
            assert_eq!(cursor, offset);
            let before = job.encode().unwrap();
            for (epoch, source, target, start, limit) in [
                (2, 131072, 147456, offset, 64),
                (1, 147456, 147456, offset, 64),
                (1, 131072, 131072, offset, 64),
                (1, 131072, 147456, offset + 1, 64),
                (1, 131072, 147456, offset, 0),
                (1, 131072, 147456, offset, 65),
                (1, 131072, 147456, u32::MAX, 64),
            ] {
                assert!(job.batch(target, source, epoch, start, limit).is_err());
                assert_eq!(job.encode().unwrap(), before);
            }
            let batch = job.batch(147456, 131072, 1, offset, 64).unwrap();
            assert!(!batch.retry);
            assert_eq!(batch.start, offset);
            assert_eq!(batch.end as u64, (u64::from(offset) + 64).min(count));
            let committed = job.encode().unwrap();
            job = Job::decode(&committed).unwrap();
            let retry = job.batch(147456, 131072, 1, offset, 64).unwrap();
            assert!(retry.retry);
            assert_eq!(retry.end, batch.end);
            assert_eq!(job.encode().unwrap(), committed);
            if batch.complete {
                assert!(job.importing(147456, 131072, 1).is_err());
                assert!(job.batch(147456, 131072, 1, offset, 63).is_err());
                assert_eq!(job.encode().unwrap(), committed);
                let next = job.export(147456, 131072, 2, 41, &[1]).unwrap();
                assert_eq!(&next.encode().unwrap()[8..10], &1u16.to_le_bytes());
                break;
            }
            assert!(job.batch(147456, 131072, 1, offset, 63).is_err());
            offset = batch.end;
        }
    }
}

#[test]
fn progress_wire_rejects_resealed_inconsistent_cursors_versions_and_phases() {
    let mut job = Job::initial(131072)
        .unwrap()
        .export(131072, 147456, 1, 41, &[7; 65])
        .unwrap();
    job.begin_import(147456, 131072, 1).unwrap();
    job.batch(147456, 131072, 1, 0, 64).unwrap();
    let bytes = job.encode().unwrap();
    for (offset, word) in [
        (8, 1),
        (10, 2),
        (10, 1),
        (44, 65),
        (48, 1),
        (52, 0),
        (52, 65),
        (56, 1),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = word;
        reseal(&mut changed);
        assert!(
            Job::decode(&changed).is_err(),
            "offset {offset}, value {word}"
        );
    }
    for length in 0..bytes.len() {
        assert!(Job::decode(&bytes[..length]).is_err());
    }
    let before = job.encode().unwrap();
    job.batch(147456, 131072, 1, 64, 64).unwrap();
    assert!(job.batch(147456, 131072, 1, 0, 64).is_err());
    assert_ne!(job.encode().unwrap(), before);
}

#[test]
fn staged_batch_sizes_can_change_only_at_the_current_cursor() {
    let mut job = Job::initial(131072)
        .unwrap()
        .export(131072, 147456, 1, 41, &[7; 65])
        .unwrap();
    job.begin_import(147456, 131072, 1).unwrap();
    for (start, limit, end) in [(0, 1, 1), (1, 7, 8), (8, 64, 65)] {
        let batch = job.batch(147456, 131072, 1, start, limit).unwrap();
        assert_eq!(
            (batch.end, batch.retry, batch.complete),
            (end, false, end == 65)
        );
        job = Job::decode(&job.encode().unwrap()).unwrap();
        assert!(job.batch(147456, 131072, 1, start, limit).unwrap().retry);
        if start != 0 {
            let before = job.encode().unwrap();
            assert!(job.batch(147456, 131072, 1, 0, 1).is_err());
            assert_eq!(job.encode().unwrap(), before);
        }
    }
}

#[test]
fn restart_discards_only_unfinished_progress_and_preserves_checkpoint_binding() {
    for values in [vec![], vec![7; 65], vec![9; 512]] {
        let mut job = Job::initial(131072).unwrap();
        assert_eq!(job.restart_import(131072, 131072, 0), Err(Error::State));
        job = job.export(131072, 147456, 1, 41, &values).unwrap();
        let sealed = job.encode().unwrap();
        assert_eq!(job.restart_import(147456, 131072, 1), Err(Error::State));
        assert_eq!(job.encode().unwrap(), sealed);
        let mut cancelled = Job::decode(&sealed).unwrap();
        cancelled.cancel(131072, 147456, 1).unwrap();
        assert_eq!(
            cancelled.restart_import(147456, 131072, 1),
            Err(Error::State)
        );
        job.begin_import(147456, 131072, 1).unwrap();
        let initial = job.encode().unwrap();
        if !values.is_empty() {
            job.batch(147456, 131072, 1, 0, 64).unwrap();
        }
        // Exercise a fresh reader of the durable progress, not just one live job.
        job = Job::decode(&job.encode().unwrap()).unwrap();
        let before = job.encode().unwrap();
        for (target, source, epoch, error) in [
            (131072, 131072, 1, Error::Domain),
            (147456, 147456, 1, Error::Domain),
            (147456, 131072, 2, Error::Epoch),
        ] {
            assert_eq!(job.restart_import(target, source, epoch), Err(error));
            assert_eq!(job.encode().unwrap(), before);
        }
        let decoded = job.restart_import(147456, 131072, 1).unwrap();
        assert_eq!(decoded.values, values);
        assert_eq!(decoded.counter, 41);
        assert_eq!(job.encode().unwrap(), initial);
        job.restart_import(147456, 131072, 1).unwrap();
        assert_eq!(job.encode().unwrap(), initial);
        assert!(job.batch(147456, 131072, 1, 64, 64).is_err());
        let mut offset = 0;
        loop {
            let batch = job.batch(147456, 131072, 1, offset, 64).unwrap();
            assert!(!batch.retry, "pre-restart acknowledgement was retired");
            if batch.complete {
                break;
            }
            offset = batch.end;
        }
        let complete = job.encode().unwrap();
        assert_eq!(job.restart_import(147456, 131072, 1), Err(Error::State));
        assert_eq!(job.encode().unwrap(), complete);
    }
}
