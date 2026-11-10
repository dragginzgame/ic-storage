use super::*;
#[test]
fn largest_matrix_responses_fit_protocol_budgets() {
    let rows: Vec<_> = (0..128).map(|id| (id, value(id, 4096, false))).collect();
    let encoded = ic_wire::native::encode_response(3, &rows, LIMITS).unwrap();
    let decoded: Vec<(u64, Vec<u8>)> =
        ic_wire::native::decode_response(3, &encoded, LIMITS).unwrap();
    assert_eq!(decoded, rows);
    let response = Response {
        decode: None,
        instructions: 0,
        stable_pages: 0,
        length: 1,
        entries: (0..32)
            .map(|id| (id, Some(value(id, 16_384, false))))
            .collect(),
    };
    let encoded = ic_wire::native::encode_response(RUN_ID, &response, LIMITS).unwrap();
    let decoded: Response = ic_wire::native::decode_response(RUN_ID, &encoded, LIMITS).unwrap();
    assert_eq!(decoded.entries, response.entries);
}
#[test]
fn traces_are_bounded_repeatable_and_missing_keys_are_disjoint() {
    for pattern in [
        Pattern::Uniform,
        Pattern::Hot16,
        Pattern::Zipf,
        Pattern::Sequential,
    ] {
        for operation in OPERATIONS {
            let r = Request {
                case: crate::case!(Map, U64, Bytes),
                operation,
                rows: 101,
                payload: 1024,
                count: 32,
                seed: 7,
                pattern,
            };
            let a = keys(r);
            assert_eq!(a, keys(r));
            assert_eq!(a.len(), 32);
            assert!(a.iter().all(|&key| {
                if matches!(
                    operation,
                    Operation::MissAbove
                        | Operation::Insert
                        | Operation::RemoveMiss
                        | Operation::RemoveInserted
                        | Operation::Push
                ) {
                    key >= 101
                } else {
                    key < if pattern == Pattern::Hot16 { 16 } else { 101 }
                }
            }));
            let bytes = ic_wire::native::encode_request(RUN_ID, &r, LIMITS).unwrap();
            let decoded: Request = ic_wire::native::decode_request(RUN_ID, &bytes, LIMITS).unwrap();
            assert_eq!(keys(decoded), a);
        }
    }
    assert_ne!(value(7, 32, false), value(7, 32, true));
}

#[test]
fn sequential_trace_wraps_and_keeps_misses_outside_population() {
    let request = Request {
        case: crate::case!(Map, U64, U64),
        operation: Operation::Get,
        rows: 5,
        payload: 8,
        count: 8,
        seed: 4,
        pattern: Pattern::Sequential,
    };
    assert_eq!(keys(request), [4, 0, 1, 2, 3, 4, 0, 1]);
    assert_eq!(
        keys(Request {
            seed: u64::MAX,
            ..request
        }),
        [0, 1, 2, 3, 4, 0, 1, 2]
    );
    assert_eq!(
        keys(Request {
            operation: Operation::MissAbove,
            ..request
        }),
        (5..13).collect::<Vec<_>>()
    );
    assert_eq!(
        keys(Request {
            operation: Operation::RemoveMiss,
            ..request
        }),
        (13..21).collect::<Vec<_>>()
    );
}

#[test]
fn insertion_and_missing_removal_ranges_are_disjoint_at_every_supported_batch_size() {
    for case in [
        crate::case!(Map, U64, Bytes),
        crate::case!(Set, String, Unit),
        crate::case!(Region, U64, Bytes),
    ] {
        for count in 1..=max_batch(case) {
            let request = Request {
                case,
                operation: Operation::Insert,
                rows: 256,
                payload: case.payloads()[0],
                count,
                seed: 7,
                pattern: Pattern::Uniform,
            };
            let inserted: std::collections::BTreeSet<_> = keys(request).into_iter().collect();
            let absent: std::collections::BTreeSet<_> = keys(Request {
                operation: Operation::RemoveMiss,
                ..request
            })
            .into_iter()
            .collect();
            assert!(inserted.is_disjoint(&absent));
            assert!(absent.iter().all(|&key| key >= u64::from(request.rows)));
        }
    }
}

#[test]
fn session_envelope_preserves_full_identity_and_rejects_malformed_outer_frames() {
    for size in crate::case!(Vector, U64, Session).payloads() {
        for key in [0, 255, 256, 2047] {
            let value = SessionEnvelope::make(key, *size, false);
            let encoded = value.encode().unwrap();
            assert!(encoded.len() <= 4096);
            assert_eq!(SessionEnvelope::decode(&encoded).unwrap(), value);
            assert_eq!(&value.canonical()[..8], &key.to_le_bytes());
            assert!(SessionEnvelope::decode(&encoded[..encoded.len() - 1]).is_err());
        }
    }
    assert!(SessionEnvelope::decode(&[]).is_err());
    for rows in crate::workloads::LOOKUP_POPULATIONS {
        for (pattern, index) in [
            (Pattern::First, 0),
            (Pattern::Middle, rows / 2),
            (Pattern::Last, rows - 1),
        ] {
            assert_eq!(
                keys(Request {
                    case: crate::case!(Vector, U64, Session),
                    operation: Operation::Get,
                    rows,
                    payload: 256,
                    count: 1,
                    seed: 7,
                    pattern
                }),
                vec![u64::from(index)]
            );
        }
    }
}

#[cfg(all(feature = "owned", feature = "dfinity"))]
mod icss_baseline;

#[cfg(all(feature = "owned", feature = "dfinity"))]
mod cell_upgrade;
