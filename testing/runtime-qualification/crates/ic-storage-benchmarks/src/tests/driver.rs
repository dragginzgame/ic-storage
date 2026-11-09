use super::*;

#[test]
fn vector_read_history_preserves_updates_and_restores_population_each_round() {
    for profile in [Workload::VectorReads, Workload::VectorOpen] {
        for &case in profile.cases() {
            let trace = phases(case, false, Some(profile));
            assert!(trace.iter().all(|(_, op)| case.operations().contains(op)));
            for &payload in profile.payloads(case) {
                let mut model: BTreeMap<_, _> = (0..u64::from(profile.rows()))
                    .map(|id| (id, case.expected_value(id, payload, false)))
                    .collect();
                let request = Request {
                    case,
                    operation: Operation::Get,
                    rows: profile.rows(),
                    payload,
                    count: 32,
                    seed: 1,
                    pattern: Pattern::Hot16,
                };
                for round in 0..profile.rounds() {
                    let mut updated = model.clone();
                    let mut replaced = round != 0;
                    for (_, operation) in &trace {
                        let response = expected(
                            &mut model,
                            Request {
                                operation: *operation,
                                ..request
                            },
                        );
                        if *operation == Operation::Get {
                            for (id, value) in response {
                                assert_eq!(value, Some(case.expected_value(id, payload, replaced)));
                            }
                        }
                        if *operation == Operation::Set {
                            replaced = true;
                            updated = model.clone();
                        }
                    }
                    assert_eq!(
                        model, updated,
                        "push/pop must preserve updated base records"
                    );
                    assert_eq!(model.len(), profile.rows() as usize);
                }
            }
        }
    }
}

#[test]
fn growth_cycles_keep_the_same_external_records_and_preserve_default_phases() {
    for &case in DEFAULT_CASES {
        assert_eq!(
            phases(case, false, None)
                .into_iter()
                .filter(|(phase, _)| phase != "GetAfterReplace")
                .map(|(_, op)| op)
                .collect::<Vec<_>>(),
            case.operations()
        );
    }
    let mut model = (0..64)
        .map(|key| {
            (
                key,
                ic_storage_benchmarks::case!(Map, U64, Bytes).expected_value(key, 33, false),
            )
        })
        .collect();
    for _ in 0..4 {
        for (phase, operation) in phases(ic_storage_benchmarks::case!(Map, U64, Bytes), true, None)
        {
            let request = Request {
                case: ic_storage_benchmarks::case!(Map, U64, Bytes),
                operation,
                rows: 64,
                payload: 33,
                count: 32,
                seed: 1,
                pattern: Pattern::Hot16,
            };
            expected(&mut model, request);
            let length = if phase == "Grow" || phase == "LargeGet" {
                132
            } else {
                33
            };
            for key in keys(request) {
                assert_eq!(model[&key].len(), length);
            }
            assert_eq!(model.len(), 64);
        }
    }
}
#[test]
fn payload_selection_is_typed_and_conflicts_with_fixed_workloads() {
    let base = [
        "bench",
        "--owned-wasm",
        "a",
        "--dfinity-wasm",
        "b",
        "--output",
        "report",
    ];
    let args = Arguments::try_parse_from(
        base.into_iter()
            .chain(["--payloads", "33,44,45,108,109,132"]),
    )
    .unwrap();
    assert_eq!(args.payloads, [33, 44, 45, 108, 109, 132]);
    assert!(
        args.payloads
            .iter()
            .all(|size| ic_storage_benchmarks::case!(Map, U64, Bytes).accepts_payload(*size))
    );
    assert!(!ic_storage_benchmarks::case!(Map, U64, U64).accepts_payload(33));
    assert!(Arguments::try_parse_from(base.into_iter().chain(["--payloads", "-1"])).is_err());
    assert!(
        Arguments::try_parse_from(base.into_iter().chain([
            "--payloads",
            "33",
            "--workload",
            "records"
        ]))
        .is_err()
    );
}
#[test]
fn named_workloads_refuse_mixed_knobs_and_preserve_operation_semantics() {
    let base = [
        "bench",
        "--owned-wasm",
        "owned",
        "--dfinity-wasm",
        "dfinity",
        "--output",
        "report",
    ];
    for name in ["control", "records", "bulk", "boundaries", "types"] {
        let args = Arguments::try_parse_from(base.into_iter().chain(["--workload", name])).unwrap();
        let workload = args.workload.unwrap();
        assert!(
            Arguments::try_parse_from(base.into_iter().chain(["--workload", name, "--rows", "7"]))
                .is_err()
        );
        for &case in workload.cases() {
            for &payload in workload.payloads(case) {
                let rows = case.rows(7);
                let mut model = (0..u64::from(rows))
                    .map(|key| (key, case.expected_value(key, payload, false)))
                    .collect();
                for operation in case.operations() {
                    let request = Request {
                        case,
                        rows,
                        payload,
                        pattern: Pattern::Uniform,
                        count: 1,
                        operation,
                        seed: 1,
                    };
                    if operation == Operation::Grow {
                        assert!(payload_for(request) > payload);
                    }
                    expected(&mut model, request);
                }
                assert_eq!(model.len(), rows as usize);
            }
        }
    }
}
#[test]
fn bucket_selection_is_nonzero_and_survives_initialization_encoding() {
    let base = [
        "bench",
        "--owned-wasm",
        "owned.wasm",
        "--dfinity-wasm",
        "dfinity.wasm",
        "--output",
        "report",
    ];
    assert_eq!(
        Arguments::try_parse_from(base)
            .unwrap()
            .upstream_bucket_pages
            .get(),
        8
    );
    for value in ["0", "65536", "-1"] {
        assert!(
            Arguments::try_parse_from(base.into_iter().chain(["--upstream-bucket-pages", value]))
                .is_err()
        );
    }
    for value in ["1", "8", "128", "65535"] {
        let args =
            Arguments::try_parse_from(base.into_iter().chain(["--upstream-bucket-pages", value]))
                .unwrap();
        let initialization = Initialization {
            case: ic_storage_benchmarks::case!(Map, U64, U64),
            upstream_cache_slots: 0,
            upstream_bucket_pages: args.upstream_bucket_pages,
            cold_cache: false,
            vector_open: false,
        };
        let bytes = ic_wire::native::encode_request(0, &initialization, LIMITS).unwrap();
        let decoded: Initialization = ic_wire::native::decode_request(0, &bytes, LIMITS).unwrap();
        assert_eq!(decoded.upstream_bucket_pages, args.upstream_bucket_pages);
    }
    #[derive(serde::Serialize)]
    struct UncheckedInitialization {
        case: Case,
        upstream_cache_slots: usize,
        upstream_bucket_pages: u16,
        cold_cache: bool,
        vector_open: bool,
    }
    for pages in [0, 1] {
        let input = UncheckedInitialization {
            case: ic_storage_benchmarks::case!(Map, U64, U64),
            upstream_cache_slots: 0,
            upstream_bucket_pages: pages,
            cold_cache: false,
            vector_open: false,
        };
        let bytes = ic_wire::native::encode_request(0, &input, LIMITS).unwrap();
        assert_eq!(
            ic_wire::native::decode_request::<Initialization>(0, &bytes, LIMITS).is_ok(),
            pages != 0
        );
    }
}
#[test]
fn full_catalogue_mutation_histories_preserve_results_and_population() {
    for &case in MATRIX_CASES {
        // Refusal/trait cases have separate codec and installed admission proofs.
        if catalogue::unavailable(case, false).is_some() {
            continue;
        }
        let rows = case.rows(7);
        let operations = case.operations();
        for &payload in case.payloads() {
            let base = Request {
                case,
                rows,
                payload,
                count: 1,
                operation: Operation::Grow,
                seed: 1,
                pattern: Pattern::Uniform,
            };
            let largest_payload = if operations.contains(&Operation::Grow) {
                payload_for(base).max(payload)
            } else {
                payload
            };
            // Preserve 32-operation histories for small values, but bound the
            // wider catalogue's composite/optional normalized results as well.
            let largest_value = [payload, largest_payload]
                .into_iter()
                .flat_map(|size| {
                    [0, 1].into_iter().flat_map(move |key| {
                        [false, true]
                            .map(|replacement| case.expected_value(key, size, replacement).len())
                    })
                })
                .max()
                .unwrap();
            let count = 32.min(u32::try_from(transfer_rows(largest_value)).unwrap());
            for pattern in [
                Pattern::Uniform,
                Pattern::Hot16,
                Pattern::Zipf,
                Pattern::Sequential,
            ] {
                let mut model: BTreeMap<_, _> = (0..u64::from(rows))
                    .map(|key| (key, case.expected_value(key, payload, false)))
                    .collect();
                for round in 0..3 {
                    for &operation in &operations {
                        let request = Request {
                            operation,
                            count,
                            pattern,
                            seed: 1 + round * 7919,
                            ..base
                        };
                        let entries = expected(&mut model, request);
                        if operation == Operation::MissAbove {
                            assert!(entries.iter().all(|(_, value)| {
                                if case.collection == Collection::Set {
                                    *value == Some(vec![0])
                                } else {
                                    value.is_none()
                                }
                            }));
                        }
                        if operation == Operation::Shrink {
                            for key in keys(request) {
                                // Composite/optional values retain other fields
                                // when their variable component becomes empty.
                                assert_eq!(model[&key], case.expected_value(key, 0, true));
                            }
                        }
                        if operation == Operation::Pop || case.collection == Collection::Cell {
                            assert_eq!(model.len(), rows as usize);
                        }
                    }
                    assert_eq!(model.len(), rows as usize, "{case:?} {pattern:?}");
                    assert!(model.keys().copied().eq(0..u64::from(rows)));
                }
            }
        }
    }
}
#[test]
fn mislabeled_removal_and_growth_are_rejected_before_execution() {
    let r = Request {
        case: ic_storage_benchmarks::case!(Map, U64, Bytes),
        rows: 7,
        payload: 33,
        pattern: Pattern::Uniform,
        operation: Operation::RemoveMiss,
        count: 1,
        seed: 1,
    };
    let mut model = BTreeMap::from([(keys(r)[0], vec![])]);
    assert!(std::panic::catch_unwind(|| admit_phase(&model, r)).is_err());
    model.clear();
    assert!(
        std::panic::catch_unwind(|| admit_phase(
            &model,
            Request {
                operation: Operation::Grow,
                ..r
            }
        ))
        .is_err()
    );
}
#[test]
fn endpoint_and_bounded_scan_models_admit_empty_and_exclude_upper_bound() {
    let mut model = BTreeMap::from([(0, vec![10]), (1, vec![11]), (2, vec![12])]);
    let mut r = Request {
        case: ic_storage_benchmarks::case!(Map, U64, U64),
        operation: Operation::BoundedScan,
        rows: 8,
        payload: 8,
        count: 4,
        seed: 0,
        pattern: Pattern::Uniform,
    };
    assert_eq!(keys(r)[0], 0);
    assert_eq!(
        expected(&mut model, r),
        vec![(0, Some(vec![10])), (1, Some(vec![11]))]
    );
    r.operation = Operation::First;
    assert_eq!(expected(&mut model, r), vec![(0, Some(vec![10])); 4]);
    r.operation = Operation::Last;
    assert_eq!(expected(&mut model, r), vec![(2, Some(vec![12])); 4]);
    model.clear();
    assert!(expected(&mut model, r).is_empty());
    r.operation = Operation::First;
    assert!(expected(&mut model, r).is_empty());
}
#[test]
fn reference_trace_handles_repeated_hot_updates_and_removals() {
    for case in DEFAULT_CASES
        .iter()
        .copied()
        .filter(|c| c.structure() == "Map")
    {
        let payload = case.payloads()[0];
        let mut model: BTreeMap<_, _> = (0..8)
            .map(|key| (key, case.expected_value(key, payload, false)))
            .collect();
        for operation in case.operations() {
            let r = Request {
                case,
                operation,
                rows: 8,
                payload,
                count: 32,
                seed: 1,
                pattern: Pattern::Hot16,
            };
            let results = expected(&mut model, r);
            if operation == Operation::MissAbove {
                assert!(results.iter().all(|(_, value)| value.is_none()));
            }
            if operation == Operation::Replace {
                assert!(results.iter().all(|(_, value)| value.is_some()));
            }
            if operation == Operation::Remove {
                assert!(results.iter().any(|(_, value)| value.is_none()));
            }
        }
    }
    assert_eq!(median(&[30, 10, 20]), 20);
}
#[test]
fn round_summary_keeps_initial_and_shrunk_cell_costs_separate() {
    let mut samples = Samples::new();
    for (backend, initial) in [("ic-storage", 100), ("ic-stable-structures", 10)] {
        samples.insert(
            (
                ic_storage_benchmarks::case!(Cell, U64, Bytes),
                1024,
                1,
                Pattern::Uniform,
                "Get".into(),
                backend.into(),
            ),
            [initial, 1, 1]
                .into_iter()
                .enumerate()
                .map(|(round, instructions)| Measurement {
                    round: u32::try_from(round).unwrap(),
                    instructions,
                    stable_pages: 4,
                })
                .collect(),
        );
    }
    let mut aggregate = Vec::new();
    let mut rounds = Vec::new();
    write_summary(
        &mut aggregate,
        &samples,
        64,
        std::num::NonZeroU16::new(8).unwrap(),
        3,
        false,
    )
    .unwrap();
    write_summary(
        &mut rounds,
        &samples,
        64,
        std::num::NonZeroU16::new(8).unwrap(),
        3,
        true,
    )
    .unwrap();
    let aggregate = String::from_utf8(aggregate).unwrap();
    let rounds = String::from_utf8(rounds).unwrap();
    assert_eq!(aggregate.lines().count(), 2);
    assert!(aggregate.lines().nth(1).unwrap().contains(",1,1,1.0000,"));
    assert!(rounds.lines().next().unwrap().ends_with(",round"));
    assert_eq!(rounds.lines().count(), 4);
    assert!(rounds.lines().nth(1).unwrap().contains(",100,10,10.0000,"));
    assert!(rounds.lines().nth(1).unwrap().ends_with(",0"));
    assert!(rounds.lines().nth(3).unwrap().ends_with(",2"));
}

#[test]
fn iterator_model_checks_skipping_mixed_ends_and_persisted_interleaved_writes() {
    let mut model: BTreeMap<_, _> = (0..8)
        .map(|id| {
            (
                id,
                ic_storage_benchmarks::case!(Vector, U64, U64).expected_value(id, 8, false),
            )
        })
        .collect();
    let request = Request {
        case: ic_storage_benchmarks::case!(Vector, U64, U64),
        rows: 8,
        payload: 8,
        count: 3,
        seed: 1,
        pattern: Pattern::Sequential,
        operation: Operation::Nth,
    };
    let ids =
        |rows: Vec<(u64, Option<Vec<u8>>)>| rows.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
    assert_eq!(ids(expected(&mut model, request)), vec![1, 2, 3]);
    assert_eq!(
        ids(expected(
            &mut model,
            Request {
                operation: Operation::NthBack,
                ..request
            }
        )),
        vec![6, 5, 4]
    );
    assert_eq!(
        ids(expected(
            &mut model,
            Request {
                operation: Operation::AlternatingScan,
                ..request
            }
        )),
        vec![0, 7, 1]
    );
    assert_eq!(
        ids(expected(
            &mut model,
            Request {
                operation: Operation::Last,
                ..request
            }
        )),
        vec![7, 7, 7]
    );
    let original = model.clone();
    let rows = expected(
        &mut model,
        Request {
            operation: Operation::ScanSet,
            count: 4,
            ..request
        },
    );
    assert_eq!(rows[0], (0, Some(original[&0].clone())));
    for id in 1..4 {
        assert_eq!(
            model[&id],
            ic_storage_benchmarks::case!(Vector, U64, U64).expected_value(id, 8, true)
        );
    }
    for id in [0, 4, 5, 6, 7] {
        assert_eq!(model[&id], original[&id]);
    }
    assert_eq!(model.len(), 8);
}

#[test]
fn ordered_iteration_model_preserves_key_identity_rank_and_range_endpoints() {
    let mut model: BTreeMap<_, _> = [2, 5, 8, 9, 12]
        .into_iter()
        .map(|key| (key, vec![key as u8]))
        .collect();
    let request = Request {
        case: ic_storage_benchmarks::case!(Map, U64, Bytes),
        operation: Operation::Nth,
        rows: 5,
        payload: 33,
        count: 3,
        seed: 0,
        pattern: Pattern::Sequential,
    };
    let original = model.clone();
    for (operation, expected_rows) in [
        (Operation::IterLast, vec![(12, Some(vec![12])); 3]),
        (Operation::KeysLast, vec![(12, Some(vec![])); 3]),
        (
            Operation::ValuesLast,
            vec![
                (0, Some(vec![12])),
                (1, Some(vec![12])),
                (2, Some(vec![12])),
            ],
        ),
    ] {
        assert_eq!(
            expected(
                &mut model,
                Request {
                    operation,
                    ..request
                }
            ),
            expected_rows
        );
        assert_eq!(model, original);
        assert!(
            expected(
                &mut BTreeMap::new(),
                Request {
                    operation,
                    ..request
                }
            )
            .is_empty()
        );
    }
    assert_eq!(
        expected(
            &mut model,
            Request {
                case: ic_storage_benchmarks::case!(Set, U64, Unit),
                operation: Operation::IterLast,
                payload: 0,
                ..request
            }
        ),
        vec![(12, Some(vec![])); 3]
    );
    assert_eq!(
        expected(&mut model, request),
        vec![(2, Some(vec![2])), (5, Some(vec![5])), (8, Some(vec![8]))]
    );
    assert_eq!(
        expected(
            &mut model,
            Request {
                operation: Operation::NthBack,
                ..request
            }
        ),
        vec![(12, Some(vec![12])), (9, Some(vec![9])), (8, Some(vec![8]))]
    );
    assert_eq!(
        expected(
            &mut model,
            Request {
                operation: Operation::ValuesScan,
                ..request
            }
        ),
        vec![(0, Some(vec![2])), (1, Some(vec![5])), (2, Some(vec![8]))]
    );
    assert_eq!(
        expected(
            &mut model,
            Request {
                operation: Operation::ReverseKeysScan,
                ..request
            }
        ),
        vec![(12, Some(vec![])), (9, Some(vec![])), (8, Some(vec![]))]
    );
    assert_eq!(
        expected(
            &mut model,
            Request {
                operation: Operation::AlternatingScan,
                ..request
            }
        ),
        vec![(2, Some(vec![2])), (12, Some(vec![12])), (5, Some(vec![5]))]
    );
    assert_eq!(
        expected(
            &mut model,
            Request {
                operation: Operation::BoundedKeysScan,
                rows: 20,
                seed: 5,
                count: 6,
                ..request
            }
        ),
        vec![(5, Some(vec![]))]
    );
}

#[test]
fn ordinary_selection_is_a_unique_subset_of_the_complete_type_suite() {
    let defaults: std::collections::BTreeSet<_> = DEFAULT_CASES.iter().copied().collect();
    let complete: std::collections::BTreeSet<_> = MATRIX_CASES.iter().copied().collect();
    assert_eq!(defaults.len(), DEFAULT_CASES.len());
    assert_eq!(complete.len(), MATRIX_CASES.len());
    assert!(defaults.is_subset(&complete));
    assert!(defaults.contains(&ic_storage_benchmarks::case!(Region, U64, Bytes)));
    assert!(!defaults.contains(&ic_storage_benchmarks::case!(Vector, U64, Bounded65536)));
    assert_eq!(Workload::Types.cases(), MATRIX_CASES);
}

#[test]
fn scalar_cell_type_trace_reads_both_original_and_replaced_optional_variants() {
    let case = Case {
        collection: Collection::Cell,
        key: DataType::U64,
        value: DataType::OptionU64,
    };
    let trace = phases(case, false, None);
    assert!(trace.contains(&("GetAfterReplace".into(), Operation::Get)));
    let mut model = [(0, case.expected_value(0, 8, false))]
        .into_iter()
        .collect();
    let mut reads = Vec::new();
    for (_, operation) in trace {
        let response = expected(
            &mut model,
            Request {
                case,
                operation,
                rows: 1,
                payload: 8,
                count: 1,
                seed: 1,
                pattern: Pattern::Uniform,
            },
        );
        if operation == Operation::Get {
            reads.push(response[0].1.clone().unwrap());
        }
    }
    assert_eq!(reads[0], vec![0]);
    assert_eq!(reads[1][0], 1);
    assert_eq!(reads[1].len(), 9);
}
#[test]
fn summary_quotes_tuple_labels_and_does_not_invent_missing_reference_costs() {
    let case = Case {
        collection: Collection::Map,
        key: DataType::PairFixed,
        value: DataType::U64,
    };
    let mut samples = Samples::new();
    samples.insert(
        (
            case,
            8,
            1,
            Pattern::Uniform,
            "Get".into(),
            "ic-storage".into(),
        ),
        vec![Measurement {
            round: 0,
            instructions: 100,
            stable_pages: 4,
        }],
    );
    let mut output = Vec::new();
    write_summary(
        &mut output,
        &samples,
        32,
        std::num::NonZeroU16::new(8).unwrap(),
        1,
        false,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\"(u64,[u8;20])\""));
    assert!(text.contains(",100,,,4,,8,"));
}
#[test]
fn combined_type_report_requires_all_five_complete_subreports() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../target/benchmark-full-catalogue/report-test")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    for (index, collection) in Collection::ALL.into_iter().enumerate() {
        let part = root.join(collection.name().to_ascii_lowercase());
        fs::create_dir_all(&part).unwrap();
        for file in [
            "summary.csv",
            "round-summary.csv",
            "coverage.csv",
            "lookup-stages.csv",
        ] {
            fs::write(part.join(file), format!("column\n{index}\n")).unwrap();
        }
        for file in ["samples.jsonl", "refusals.jsonl"] {
            fs::write(part.join(file), "{}\n").unwrap();
        }
        fs::write(part.join("configuration.json"), "{}").unwrap();
        combine_collections(&root).unwrap();
        assert_eq!(root.join("summary.csv").exists(), index == 4);
    }
    assert_eq!(
        fs::read_to_string(root.join("summary.csv"))
            .unwrap()
            .lines()
            .count(),
        6
    );
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("configuration.json")).unwrap()).unwrap();
    assert_eq!(config["status"], "complete");
    assert_eq!(config["collections"].as_array().unwrap().len(), 5);
}

#[test]
fn population_reports_distinguish_singleton_cells_in_csv_and_raw_output() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../target/evidence/storage-population-report-tests")
        .join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    let mut reports = Vec::new();
    for population in [256, 4096] {
        let part = root.join(format!("rows-{population}-update"));
        fs::create_dir_all(&part).unwrap();
        for file in ["summary.csv", "round-summary.csv", "lookup-stages.csv"] {
            fs::write(part.join(file), "rows,structure\n1,Cell\n").unwrap();
        }
        fs::write(part.join("samples.jsonl"), "{\"request\":{\"rows\":1}}\n").unwrap();
        reports.push((part, "update", population));
    }
    combine_populations(&root, &reports, false).unwrap();
    for file in ["summary.csv", "round-summary.csv", "lookup-stages.csv"] {
        assert_eq!(
            fs::read_to_string(root.join(file)).unwrap(),
            "rows,structure,execution_mode,population_context_rows\n1,Cell,update,256\n1,Cell,update,4096\n"
        );
    }
    let raw = fs::read_to_string(root.join("samples.jsonl")).unwrap();
    let records: Vec<serde_json::Value> = raw
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    for (record, population) in records.iter().zip([256, 4096]) {
        assert_eq!(record["request"]["rows"], 1);
        assert_eq!(record["execution_mode"], "update");
        assert_eq!(record["population_context_rows"], population);
    }
    for file in ["execution-summary.csv", "execution-round-summary.csv"] {
        assert!(!root.join(file).exists());
        for (part, _, _) in &reports {
            fs::write(part.join(file), "rows,cost\n1,123\n").unwrap();
        }
    }
    combine_populations(&root, &reports, true).unwrap();
    for file in ["execution-summary.csv", "execution-round-summary.csv"] {
        assert_eq!(
            fs::read_to_string(root.join(file)).unwrap(),
            "rows,cost,execution_mode,population_context_rows\n1,123,update,256\n1,123,update,4096\n"
        );
    }
}

#[test]
fn stable_read_trace_rejects_overflow_and_truncation() {
    let mut bytes = vec![0; 8 + 160 * 32 + 40];
    bytes[5128..5136].copy_from_slice(&2_u64.to_le_bytes());
    bytes[..8].copy_from_slice(&1_u64.to_le_bytes());
    for (i, value) in [65535_u64, 1048576, 4096, 80_042].into_iter().enumerate() {
        bytes[8 + i * 8..16 + i * 8].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        decode_read_trace(&bytes).unwrap().reads,
        [[65535, 1048576, 4096, 80_042]]
    );
    assert!(decode_read_trace(&bytes[..bytes.len() - 1]).is_err());
    bytes[..8].copy_from_slice(&161_u64.to_le_bytes());
    assert!(decode_read_trace(&bytes).is_err());
    bytes[..8].fill(0);
    assert!(decode_read_trace(&bytes).unwrap().reads.is_empty());
    bytes[5128..5136].fill(0);
    assert!(decode_read_trace(&bytes).is_err());
}

#[test]
fn allocator_trace_admits_its_capacity_and_rejects_partial_records() {
    let mut bytes = vec![0_u8; 8 + 64 * 32];
    bytes[..8].copy_from_slice(&64_u64.to_le_bytes());
    bytes[8..16].copy_from_slice(&8_659_336_u64.to_le_bytes());
    let rows = decode_trace_rows(&bytes, 64).unwrap();
    assert_eq!(rows.len(), 64);
    assert_eq!(rows[0][0], 8_659_336);
    assert!(decode_trace_rows(&bytes[..bytes.len() - 1], 64).is_err());
    bytes[..8].copy_from_slice(&65_u64.to_le_bytes());
    assert!(decode_trace_rows(&bytes, 64).is_err());
}

#[test]
fn execution_boundary_and_operation_probes_cannot_be_mixed() {
    let base = [
        "bench",
        "--owned-wasm",
        "a",
        "--dfinity-wasm",
        "b",
        "--output",
        "report",
    ];
    for workload in [
        "vector-reads",
        "vector-open",
        "depth",
        "auth-lookups",
        "ordered",
        "bulk",
    ] {
        for diagnostic in ["--trace-stable-reads", "--trace-execution-cost"] {
            let args = Arguments::try_parse_from(base.into_iter().chain([
                "--workload",
                workload,
                diagnostic,
            ]))
            .unwrap();
            assert_eq!(
                validate_diagnostics(&args).is_ok(),
                matches!(workload, "vector-reads" | "depth")
                    || (matches!(workload, "ordered" | "vector-open" | "bulk")
                        && diagnostic == "--trace-execution-cost")
            );
        }
    }
    for diagnostic in ["--trace-stable-reads", "--trace-execution-cost"] {
        let args = Arguments::try_parse_from(base.into_iter().chain([
            "--cases",
            "Vector-U64-U8,Vector-U64-Bool,Cell-U64-Bool",
            diagnostic,
        ]))
        .unwrap();
        assert_eq!(
            validate_diagnostics(&args).is_ok(),
            diagnostic == "--trace-execution-cost"
        );
    }
    assert!(
        Arguments::try_parse_from(
            base.into_iter()
                .chain(["--trace-execution-cost", "--trace-stable-reads",])
        )
        .is_err()
    );
    assert!(
        Arguments::try_parse_from(base.into_iter().chain(["--trace-allocator-accesses",])).is_err()
    );
    assert!(
        Arguments::try_parse_from(base.into_iter().chain([
            "--trace-stable-reads",
            "--trace-allocator-accesses",
            "--workload",
            "vector-reads",
        ]))
        .is_ok()
    );
}

#[test]
fn session_lookup_scope_preserves_full_catalogue_mutations() {
    let case = ic_storage_benchmarks::case!(Vector, U64, Session);
    for operation in [
        Operation::Set,
        Operation::Push,
        Operation::Pop,
        Operation::Scan,
    ] {
        assert!(case.operations().contains(&operation));
        assert!(!case.measures_decode(operation));
    }
    let actual: Vec<_> = phases(case, false, Some(Workload::AuthLookups))
        .into_iter()
        .map(|(_, operation)| operation)
        .collect();
    assert_eq!(
        actual,
        [
            Operation::Control,
            Operation::Get,
            Operation::WarmGet,
            Operation::Decode,
            Operation::MissAbove
        ]
    );
    assert!(
        actual
            .into_iter()
            .all(|operation| case.measures_decode(operation))
    );
}

#[test]
fn bulk_read_probes_require_the_bounded_vector_shard() {
    for collection in ["vector", "map", "cell", "set", "region"] {
        let args = Arguments::try_parse_from([
            "bench",
            "--owned-wasm",
            "a",
            "--dfinity-wasm",
            "b",
            "--output",
            "report",
            "--workload",
            "bulk",
            "--collection",
            collection,
            "--trace-stable-reads",
            "--trace-allocator-accesses",
        ])
        .unwrap();
        assert_eq!(validate_diagnostics(&args).is_ok(), collection == "vector");
    }
}
