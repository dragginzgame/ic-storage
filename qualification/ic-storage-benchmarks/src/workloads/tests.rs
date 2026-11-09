use super::*;
use crate::{LIMITS, Operation, RUN_ID, Request, Response, payload_for, value};

#[test]
fn every_profile_result_fits_the_existing_wire_budget() {
    for profile in [
        Workload::Control,
        Workload::Records,
        Workload::Bulk,
        Workload::Boundaries,
        Workload::Iterators,
        Workload::Ordered,
        Workload::AuthLookups,
        Workload::Types,
        Workload::Depth,
        Workload::VectorReads,
        Workload::VectorOpen,
    ] {
        for &case in profile.cases() {
            for &payload in profile.payloads(case) {
                assert!(case.accepts_payload(payload));
                let count = *profile.counts().last().unwrap();
                let response = Response {
                    decode: None,
                    instructions: 0,
                    stable_pages: 0,
                    length: u64::from(count),
                    entries: (0..u64::from(count))
                        .map(|id| (id, Some(case.expected_value(id, payload, false))))
                        .collect(),
                };
                let encoded = ic_wire::native::encode_response(RUN_ID, &response, LIMITS).unwrap();
                assert!(encoded.len() <= 2 * 1024 * 1024, "IC reply ceiling");
                let decoded: Response =
                    ic_wire::native::decode_response(RUN_ID, &encoded, LIMITS).unwrap();
                assert_eq!(decoded.entries, response.entries);
                if case.operations().contains(&Operation::Grow) {
                    let grown = payload_for(Request {
                        case,
                        operation: Operation::Grow,
                        rows: profile.rows(),
                        payload,
                        count,
                        seed: 1,
                        pattern: Pattern::Uniform,
                    });
                    assert!(grown > payload, "{case:?}: {payload} -> {grown}");
                    let response = Response {
                        entries: (0..u64::from(count))
                            .map(|id| (id, Some(case.expected_value(id, grown, true))))
                            .collect(),
                        ..response
                    };
                    let encoded = ic_wire::native::encode_response(RUN_ID, &response, LIMITS)
                        .unwrap_or_else(|e| panic!("{case:?}: {payload} -> {grown}: {e:?}"));
                    assert!(encoded.len() <= 2 * 1024 * 1024, "IC reply ceiling");
                    let decoded: Response =
                        ic_wire::native::decode_response(RUN_ID, &encoded, LIMITS).unwrap();
                    assert_eq!(decoded.entries, response.entries);
                }
            }
        }
    }
    let rows: Vec<_> = (0..transfer_rows(1_572_864))
        .map(|id| (id as u64, value(id as u64, 1_572_864, false)))
        .collect();
    let bytes = ic_wire::native::encode_response(3, &rows, LIMITS).unwrap();
    assert!(bytes.len() <= 2 * 1024 * 1024, "IC reply ceiling");
    let decoded: Vec<(u64, Vec<u8>)> = ic_wire::native::decode_response(3, &bytes, LIMITS).unwrap();
    assert_eq!(rows, decoded);
}

#[test]
fn depth_profile_covers_all_forms_and_population_mutation_histories() {
    let profile = Workload::Depth;
    assert_eq!(DEPTH_POPULATIONS, [256, 4096]);
    for collection in Collection::ALL {
        assert!(
            profile
                .cases()
                .iter()
                .any(|case| case.structure() == collection.name())
        );
    }
    assert_eq!(profile.counts(), &[1, 32]);
    assert_eq!(profile.patterns(), &[Pattern::Uniform, Pattern::Hot16]);
    assert!(profile.rounds() > 1);
    for case in profile.cases() {
        let operations = case.operations();
        assert!(operations.contains(&Operation::Get));
        if case.structure() == "Map" {
            for phase in [Operation::Insert, Operation::Remove, Operation::Reinsert] {
                assert!(operations.contains(&phase));
            }
        }
    }
    assert!(
        crate::case!(Map, U64, Bytes)
            .operations()
            .contains(&Operation::Grow)
    );
    assert!(
        crate::case!(Map, U64, Bytes)
            .operations()
            .contains(&Operation::Shrink)
    );
}

#[test]
fn iterator_profile_exercises_boundary_counts_and_complete_vector_operation_sets() {
    assert_eq!(
        Workload::Iterators.counts(),
        &[1, 2, 31, 32, 63, 64, 65, 127, 128, 129, 256]
    );
    for &case in Workload::Iterators.cases() {
        assert_eq!(case.structure(), "Vector");
        for &count in Workload::Iterators.counts() {
            let request = Request {
                case,
                operation: Operation::NthBack,
                rows: Workload::Iterators.rows(),
                payload: Workload::Iterators.payloads(case)[0],
                count,
                seed: 7,
                pattern: Pattern::Uniform,
            };
            let trace = crate::keys(request);
            assert_eq!(trace.len(), count as usize);
            assert!(trace.iter().all(|&key| key < u64::from(request.rows)));
        }
        for operation in [
            Operation::Nth,
            Operation::NthBack,
            Operation::Last,
            Operation::AlternatingScan,
            Operation::ScanSet,
        ] {
            assert!(case.operations().contains(&operation));
        }
    }
    let invalid = Request {
        case: crate::case!(Vector, U64, U64),
        operation: Operation::Scan,
        rows: 256,
        payload: 8,
        count: 257,
        seed: 1,
        pattern: Pattern::Uniform,
    };
    assert!(std::panic::catch_unwind(|| crate::keys(invalid)).is_err());
    let ordinary_map = Request {
        case: crate::case!(Map, U64, U64),
        count: 129,
        ..invalid
    };
    assert!(std::panic::catch_unwind(|| crate::keys(ordinary_map)).is_err());
}

#[test]
fn ordered_profile_covers_node_boundaries_inline_transitions_and_supported_set_operations() {
    assert_eq!(
        Workload::Ordered.counts(),
        &[1, 5, 6, 11, 12, 31, 32, 64, 128]
    );
    assert_eq!(
        Workload::Ordered.payloads(crate::case!(Map, U64, Bytes)),
        &[31, 32, 33, 256]
    );
    for &case in Workload::Ordered.cases() {
        assert!(matches!(case.structure(), "Map" | "Set"));
        for &count in Workload::Ordered.counts() {
            assert!(count <= crate::max_batch(case));
        }
        assert!(case.operations().contains(&Operation::Nth));
        assert!(case.operations().contains(&Operation::IterLast));
        if case.structure() == "Map" {
            for op in [
                Operation::NthBack,
                Operation::ValuesScan,
                Operation::ReverseKeysScan,
                Operation::BoundedKeysScan,
                Operation::AlternatingScan,
                Operation::KeysLast,
                Operation::ValuesLast,
            ] {
                assert!(case.operations().contains(&op));
            }
        } else {
            // ICSS's public Set iterator is forward-only; paired timings use its actual API.
            for op in [
                Operation::ReverseScan,
                Operation::NthBack,
                Operation::AlternatingScan,
            ] {
                assert!(!case.operations().contains(&op));
            }
        }
    }
}
