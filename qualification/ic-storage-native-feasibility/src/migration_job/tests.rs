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
            (10, 3),
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
