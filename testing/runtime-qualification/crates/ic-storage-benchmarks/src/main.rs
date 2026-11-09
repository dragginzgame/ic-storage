//! Installed, matched map comparison. Testkit owns the explicitly prepared server.
use clap::Parser;
use ic_storage_benchmarks::workloads::{
    DEPTH_POPULATIONS, LOOKUP_POPULATIONS, Workload, transfer_rows,
};
use ic_storage_benchmarks::*;
use ic_testkit::pocket_ic::{CanisterId, PocketIcBuilder};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};

#[derive(Clone, Parser)]
#[command(about = "Compare original Wasm map workloads on a prepared PocketIC server")]
struct Arguments {
    /// Fixed consumer workload; owns cases, sizes, population, batching and access patterns.
    #[arg(long, value_enum, conflicts_with_all = ["rows", "cases", "payloads", "growth_cycles", "patterns", "shuffled", "upstream_cache_slots", "cold_cache", "rounds"])]
    workload: Option<Workload>,
    /// Type-suite or bulk-workload shard; ordinary Make runs retain all collections.
    #[arg(long, value_enum)]
    collection: Option<Collection>,
    #[arg(skip)]
    query: bool,
    /// Diagnostic modules only: collect stable-read traces and timed heap counters.
    #[arg(long)]
    trace_stable_reads: bool,
    /// Second diagnostic pass: collect costly allocator memory accesses.
    #[arg(long, requires = "trace_stable_reads")]
    trace_allocator_accesses: bool,
    /// Boundary-only modules: collect the full benchmark handler's instruction cost.
    #[arg(long, conflicts_with = "trace_stable_reads")]
    trace_execution_cost: bool,
    #[arg(long)]
    owned_wasm: PathBuf,
    #[arg(long)]
    dfinity_wasm: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 256, value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
    rows: u32,
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=20))]
    samples: u32,
    /// Optional concrete case names, e.g. Map-U64-U64,Vector-U64-U64.
    #[arg(long, value_delimiter = ',')]
    cases: Vec<String>,
    /// Select admitted payload sizes instead of each case's default matrix.
    #[arg(long, value_delimiter = ',')]
    payloads: Vec<u32>,
    /// Repeat small replacement/read and fourfold growth/read on the same keys.
    /// Restricted to Map-U64-Bytes; does not remove records or shrink them to empty.
    #[arg(long)]
    growth_cycles: bool,
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "uniform,hot16"
    )]
    patterns: Vec<Pattern>,
    #[arg(long)]
    shuffled: bool,
    #[arg(long, default_value_t = 0)]
    upstream_cache_slots: usize,
    /// ICSS MemoryManager bucket size, in 64 KiB pages; does not affect Storage.
    #[arg(long, default_value = "8")]
    upstream_bucket_pages: std::num::NonZeroU16,
    #[arg(long)]
    cold_cache: bool,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=20))]
    rounds: u32,
    #[arg(long, default_value = "0.7.2")]
    reference_revision: String,
}
struct Measurement {
    round: u32,
    instructions: u64,
    stable_pages: u64,
}
type Samples = BTreeMap<(Case, u32, u32, Pattern, String, String), Vec<Measurement>>;
fn phases(case: Case, growth_cycles: bool, workload: Option<Workload>) -> Vec<(String, Operation)> {
    if matches!(workload, Some(Workload::AuthLookups)) {
        return [
            Operation::Control,
            Operation::Get,
            Operation::WarmGet,
            Operation::Decode,
            Operation::MissAbove,
        ]
        .into_iter()
        .map(|operation| (format!("{operation:?}"), operation))
        .collect();
    }
    if matches!(workload, Some(Workload::VectorReads | Workload::VectorOpen)) {
        assert!(!growth_cycles && case.structure() == "Vector");
        let mut phases: Vec<_> = [
            ("Get", Operation::Get),
            ("Set", Operation::Set),
            ("GetAfterSet", Operation::Get),
            ("Push", Operation::Push),
            ("GetAfterPush", Operation::Get),
            ("Pop", Operation::Pop),
            ("GetAfterPop", Operation::Get),
        ]
        .into_iter()
        .map(|(name, op)| (name.into(), op))
        .collect();
        if matches!(workload, Some(Workload::VectorOpen)) {
            phases.insert(0, ("OpenControl".into(), Operation::Control));
        }
        return phases;
    }
    if growth_cycles {
        return [
            ("SmallReplace", Operation::Replace),
            ("SmallGet", Operation::Get),
            ("Grow", Operation::Grow),
            ("LargeGet", Operation::Get),
        ]
        .into_iter()
        .map(|(name, op)| (name.into(), op))
        .collect();
    }
    if matches!(
        case,
        Case {
            collection: Collection::Cell,
            ..
        }
    ) {
        let mut result = Vec::new();
        for operation in case.operations() {
            result.push((format!("{operation:?}"), operation));
            if operation == Operation::Replace {
                result.push(("GetAfterReplace".into(), Operation::Get));
            }
        }
        return result;
    }
    case.operations()
        .into_iter()
        .map(|op| (format!("{op:?}"), op))
        .collect()
}
#[derive(Debug, serde::Serialize)]
struct ReadTrace {
    reads: Vec<[u64; 4]>,
    // Measurement counter calls, then malloc/free call counts and inclusive costs.
    heap: [u64; 5],
}

// The diagnostic injector stores 160 (destination, source, length, instructions)
// records in Wasm globals. Reject truncation rather than interpreting a partial trace.
fn decode_read_trace(bytes: &[u8]) -> Result<ReadTrace, &'static str> {
    if bytes.len() != 8 + 160 * 32 + 40 {
        return Err("invalid stable-read trace size");
    }
    let heap = std::array::from_fn(|i| {
        u64::from_le_bytes(bytes[5128 + i * 8..5136 + i * 8].try_into().unwrap())
    });
    if heap[0] != 2 {
        return Err("expected exactly two measurement counters");
    }
    let reads = decode_trace_rows(&bytes[..5128], 160)?;
    Ok(ReadTrace { reads, heap })
}

fn decode_trace_rows(bytes: &[u8], capacity: usize) -> Result<Vec<[u64; 4]>, &'static str> {
    if bytes.len() != 8 + capacity * 32 {
        return Err("invalid trace size");
    }
    let count = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    if count > capacity as u64 {
        return Err("trace overflow");
    }
    Ok(bytes[8..]
        .as_chunks::<32>()
        .0
        .iter()
        .take(count as usize)
        .map(|row| {
            std::array::from_fn(|i| u64::from_le_bytes(row[i * 8..i * 8 + 8].try_into().unwrap()))
        })
        .collect())
}

fn validate_diagnostics(args: &Arguments) -> Result<(), &'static str> {
    if !matches!(args.workload, Some(Workload::VectorReads | Workload::Depth)) {
        if args.trace_stable_reads
            && !(matches!(args.workload, Some(Workload::Bulk))
                && matches!(args.collection, Some(Collection::Vector)))
        {
            return Err("read probes require vector-reads, depth or bulk --collection vector");
        }
        if args.trace_execution_cost
            && !matches!(
                args.workload,
                None | Some(Workload::Ordered | Workload::VectorOpen | Workload::Bulk)
            )
        {
            return Err(
                "execution probes require custom cases or --workload vector-reads, vector-open, depth, ordered or bulk",
            );
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    validate_diagnostics(&args)?;
    if args.collection.is_some() && !matches!(args.workload, Some(Workload::Types | Workload::Bulk))
    {
        return Err("--collection requires --workload types or bulk".into());
    }
    if matches!(
        args.workload,
        Some(Workload::AuthLookups | Workload::Depth | Workload::VectorOpen)
    ) {
        let auth = matches!(args.workload, Some(Workload::AuthLookups));
        let populations: &[u32] = if auth {
            &LOOKUP_POPULATIONS
        } else {
            &DEPTH_POPULATIONS
        };
        let modes: &[(bool, &str)] = if auth {
            &[(false, "update"), (true, "query")]
        } else {
            &[(false, "update")]
        };
        fs::create_dir(&args.output)?;
        let mut reports = Vec::new();
        for &rows in populations {
            for &(query, mode) in modes {
                let mut selected = args.clone();
                selected.rows = rows;
                selected.query = query;
                selected.output = args.output.join(format!("rows-{rows}-{mode}"));
                reports.push((selected.output.clone(), mode, rows));
                run(selected).await?;
            }
        }
        combine_populations(&args.output, &reports, args.trace_execution_cost)?;
        fs::write(
            args.output.join("configuration.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"workload":args.workload, "populations":populations,"subreports":reports,"execution_modes":modes.iter().map(|(_,mode)|mode).collect::<Vec<_>>(),"instrumented":auth || args.trace_stable_reads || args.trace_execution_cost,"scope":if auth {"outer Wire envelope only; opaque inner Auth bytes"} else {"collection operations; population, hot access, growth and churn controls"}}),
            )?,
        )?;
        return Ok(());
    }
    if let Some(collection) = args.collection {
        let root = args.output.clone();
        fs::create_dir_all(&root)?;
        let mut selected = args;
        selected.output = root.join(collection.name().to_ascii_lowercase());
        run(selected).await?;
        combine_collections(&root)?;
        Ok(())
    } else {
        run(args).await
    }
}
// A Cell has one value in every context. Preserve the enclosing population
// separately so aggregate rows/raw observations do not become indistinguishable.
fn combine_populations(
    root: &std::path::Path,
    reports: &[(PathBuf, &str, u32)],
    execution_cost: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = vec!["summary.csv", "round-summary.csv", "lookup-stages.csv"];
    if execution_cost {
        files.extend(["execution-summary.csv", "execution-round-summary.csv"]);
    }
    for name in files {
        let mut output = fs::File::create(root.join(name))?;
        let mut header = None;
        for (report, mode, rows) in reports {
            let text = fs::read_to_string(report.join(name))?;
            let mut lines = text.lines();
            let first = lines.next().ok_or("empty report")?;
            if let Some(previous) = &header {
                assert_eq!(previous, first);
            } else {
                writeln!(output, "{first},execution_mode,population_context_rows")?;
                header = Some(first.to_owned());
            }
            for line in lines {
                writeln!(output, "{line},{mode},{rows}")?;
            }
        }
    }
    // JSON values stream many small writes; buffer large population reports.
    let mut raw = std::io::BufWriter::new(fs::File::create(root.join("samples.jsonl"))?);
    for (report, mode, rows) in reports {
        use std::io::BufRead as _;
        let input = std::io::BufReader::new(fs::File::open(report.join("samples.jsonl"))?);
        for line in input.lines() {
            let mut record: serde_json::Value = serde_json::from_str(&line?)?;
            let fields = record.as_object_mut().ok_or("expected raw record")?;
            fields.insert("execution_mode".into(), serde_json::json!(mode));
            fields.insert("population_context_rows".into(), serde_json::json!(rows));
            writeln!(raw, "{record}")?;
        }
    }
    raw.flush()?;
    Ok(())
}
async fn run(mut args: Arguments) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(workload) = args.workload {
        if !matches!(
            workload,
            Workload::AuthLookups | Workload::Depth | Workload::VectorOpen
        ) {
            args.rows = workload.rows();
        }
        args.cases = workload.cases().iter().map(|case| case.label()).collect();
        args.patterns = workload.patterns().to_vec();
        args.shuffled = matches!(
            workload,
            Workload::Records
                | Workload::Boundaries
                | Workload::Ordered
                | Workload::Types
                | Workload::Depth
        );
        args.rounds = workload.rounds();
    }
    let batches: &[u32] = args.workload.map_or(&[1, 32], Workload::counts);
    let payloads = |case: Case| {
        if !args.payloads.is_empty() {
            return args.payloads.as_slice();
        }
        args.workload
            .map_or_else(|| case.payloads(), |workload| workload.payloads(case))
    };
    let catalogue = if matches!(args.workload, Some(Workload::Types)) {
        MATRIX_CASES
    } else if args.cases.is_empty() {
        DEFAULT_CASES
    } else {
        MATRIX_CASES
    };
    let cases: Vec<_> = catalogue
        .iter()
        .copied()
        .filter(|case| args.cases.is_empty() || args.cases.contains(&case.label()))
        .collect();
    assert!(
        !cases.is_empty()
            && args
                .cases
                .iter()
                .all(|name| cases.iter().any(|case| name == &case.label())),
        "unknown or empty case selection"
    );
    assert!(
        cases.iter().all(|case| payloads(*case)
            .iter()
            .all(|size| case.accepts_payload(*size))),
        "payload selection must be admitted by every selected case"
    );
    assert!(
        !args.growth_cycles || cases == [ic_storage_benchmarks::case!(Map, U64, Bytes)],
        "growth cycles require only Map-U64-Bytes"
    );
    let cases: Vec<_> = cases
        .into_iter()
        .filter(|case| args.collection.is_none_or(|c| case.structure() == c.name()))
        .collect();
    assert!(!cases.is_empty(), "empty collection shard");
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL")?;
    let wasm = [fs::read(&args.owned_wasm)?, fs::read(&args.dfinity_wasm)?];
    fs::create_dir(&args.output)?;
    fs::write(
        args.output.join("configuration.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "instrumented_stable_reads": args.trace_stable_reads,
            "instrumented_allocator_accesses": args.trace_allocator_accesses,
            "instrumented_execution_cost": args.trace_execution_cost,
            "vector_open": matches!(args.workload, Some(Workload::VectorOpen)),
            "vector_open_measurement": "allocation owner retained; lease/Vector opening, existing operation preparation/result conversion and Vector/lease drop inside the timer; initialization separately includes the complete init handler",
            "read_trace_batch_limit": (args.trace_stable_reads && matches!(args.workload, Some(Workload::Depth))).then_some(1),
            "execution_measurement": args.trace_execution_cost.then_some("exported benchmark handler including request decoding, input preparation, collection loop, response encoding/reply and cleanup; includes boundary probe overhead; excludes installation/population and host execution setup"),
            "execution_mode": if args.query {"query"} else {"update"}, "rows": args.rows, "structures": ["Map","Set","Vector","Cell","Region"], "samples": args.samples, "cases": cases.iter().map(|case| serde_json::json!({"case":case, "key_type":case.key_type(), "value_type":case.value_type(), "key_bound":case.key_bound(), "value_bound":case.value_bound(), "logical_value_bytes":payloads(*case), "structure":case.structure(), "operations":case.operations(), "phases":phases(*case,args.growth_cycles,args.workload), "payload_scope":case.value.payload_scope(), "population_rows":case.rows(args.rows)})).collect::<Vec<_>>(), "batch_sizes": batches, "workload": args.workload,
            "patterns": args.patterns, "seed": if args.growth_cycles { "sample + 1 (same keys across rounds)" } else { "sample + 1 + round * 7919" }, "shuffled": args.shuffled,
            "growth_cycles": args.growth_cycles, "phase_override": args.growth_cycles.then(|| phases(ic_storage_benchmarks::case!(Map, U64, Bytes), true, args.workload).into_iter().map(|(name,_)|name).collect::<Vec<_>>()),
            "upstream_cache_slots": args.upstream_cache_slots, "cold_cache": args.cold_cache, "cache_scope": "Storage has no node cache; optional Git reference ordinary Map only", "rounds": args.rounds, "reference_revision": args.reference_revision,
            "upstream_bucket_pages": args.upstream_bucket_pages,
            "backend_order": "alternates each sample", "measurement": if args.trace_stable_reads { "instrumented diagnostic: operation counts include probe overhead; stable-read records cover the entire call, allocator counters cover the timed loop; not production timings" } else { "performance_counter(0) around operations; setup, trace/value creation, validation and Wire response excluded" },
            "owned_wasm_bytes": module_size(&wasm[0]), "dfinity_wasm_bytes": module_size(&wasm[1]),
            "install_payload_bytes": [wasm[0].len(),wasm[1].len()], "install_encoding": [install_encoding(&wasm[0]),install_encoding(&wasm[1])],
            "state": "fresh canister per case/sample/backend; operations run in declared order"
        }))?,
    )?;
    let pic = PocketIcBuilder::new(std::time::Duration::from_secs(300))
        .with_server_url(url.parse()?)
        .with_application_subnet()
        .build()
        .await?;
    let mut raw = fs::File::create(args.output.join("samples.jsonl"))?;
    let mut stages = fs::File::create(args.output.join("lookup-stages.csv"))?;
    writeln!(
        stages,
        "rows,payload,pattern,phase,backend,round,total_instructions,decoder_instructions,decoder_calls,decoder_input_bytes"
    )?;
    let mut coverage = fs::File::create(args.output.join("coverage.csv"))?;
    writeln!(
        coverage,
        "case,collection,key_type,value_type,key_source,value_source,backend,status,phase,reason"
    )?;
    let mut refusals = fs::File::create(args.output.join("refusals.jsonl"))?;
    let mut timings: Samples = BTreeMap::new();
    let mut execution_timings: Samples = BTreeMap::new();
    for case in cases {
        for backend in 0..2 {
            let name = ["ic-storage", "ic-stable-structures"][backend];
            let limitation = catalogue::unavailable(case, backend == 1);
            if let Some((phase, reason)) = limitation {
                if phase != "trait" {
                    let error = probe_refusal(
                        &pic,
                        &wasm[backend],
                        case,
                        args.upstream_bucket_pages,
                        phase,
                    )
                    .await?;
                    writeln!(
                        refusals,
                        "{}",
                        serde_json::json!({"case":case,"backend":name,"phase":phase,"error":error})
                    )?;
                }
                writeln!(
                    coverage,
                    "{},{},{},{},{},{},{name},{},{phase},{}",
                    csv_field(&case.label()),
                    case.structure(),
                    csv_field(case.key_type()),
                    csv_field(case.value_type()),
                    case_sources(case).0,
                    case_sources(case).1,
                    if phase == "trait" {
                        "not-applicable"
                    } else {
                        "refused"
                    },
                    csv_field(reason)
                )?;
            } else {
                writeln!(
                    coverage,
                    "{},{},{},{},{},{},{name},supported,,",
                    csv_field(&case.label()),
                    case.structure(),
                    csv_field(case.key_type()),
                    csv_field(case.value_type()),
                    case_sources(case).0,
                    case_sources(case).1
                )?;
            }
            coverage.flush()?;
        }
        for &payload in payloads(case) {
            for &count in batches {
                for &pattern in &args.patterns {
                    for sample in 0..args.samples {
                        for backend in if sample % 2 == 0 { [0, 1] } else { [1, 0] } {
                            if catalogue::unavailable(case, backend == 1).is_some() {
                                continue;
                            }
                            let name = ["ic-storage", "ic-stable-structures"][backend];
                            let canister = pic.create_canister().await?;
                            pic.add_cycles(canister, 100_000_000_000_000).await?;
                            pic.install_canister(
                                canister,
                                wasm[backend].clone(),
                                ic_wire::native::encode_request(
                                    0,
                                    &Initialization {
                                        case,
                                        upstream_cache_slots: args.upstream_cache_slots,
                                        upstream_bucket_pages: args.upstream_bucket_pages,
                                        cold_cache: args.cold_cache,
                                        vector_open: matches!(
                                            args.workload,
                                            Some(Workload::VectorOpen)
                                        ),
                                    },
                                    LIMITS,
                                )?,
                                None,
                            )
                            .await?;
                            if matches!(args.workload, Some(Workload::VectorOpen)) {
                                let bytes = pic
                                    .query_call(
                                        canister,
                                        CanisterId::anonymous(),
                                        "initialization_cost",
                                        ic_wire::native::encode_request(INIT_COST_ID, &(), LIMITS)?,
                                    )
                                    .await?;
                                let instructions: u64 =
                                    ic_wire::native::decode_response(INIT_COST_ID, &bytes, LIMITS)?;
                                let mut output = fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(args.output.join("initialization.jsonl"))?;
                                writeln!(
                                    output,
                                    "{}",
                                    serde_json::json!({"backend":name,"case":case.label(),"payload":payload,"rows":args.rows,"batch":count,"pattern":pattern,"sample":sample,"instructions":instructions})
                                )?;
                            }
                            let ids = population(
                                case.rows(args.rows),
                                args.shuffled
                                    && matches!(case.structure(), "Map" | "Set" | "Region"),
                                u64::from(sample) + 1,
                            );
                            for start in
                                (0..case.rows(args.rows)).step_by(transfer_rows(payload as usize))
                            {
                                let count = (case.rows(args.rows) - start)
                                    .min(transfer_rows(payload as usize) as u32);
                                let setup = Setup {
                                    start: u64::from(start),
                                    count,
                                    payload,
                                    ids: ids[start as usize..(start + count) as usize].to_vec(),
                                };
                                let reply = pic
                                    .update_call(
                                        canister,
                                        CanisterId::anonymous(),
                                        "setup",
                                        ic_wire::native::encode_request(SETUP_ID, &setup, LIMITS)?,
                                    )
                                    .await
                                    .map_err(|e| format!("{name} setup: {e:?}"))?;
                                let length: u64 =
                                    ic_wire::native::decode_response(SETUP_ID, &reply, LIMITS)?;
                                assert_eq!(length, u64::from(start + setup.count));
                            }
                            let mut model: BTreeMap<_, _> = (0..u64::from(case.rows(args.rows)))
                                .map(|key| (key, case.expected_value(key, payload, false)))
                                .collect();
                            for round in 0..args.rounds {
                                for (phase, operation) in
                                    phases(case, args.growth_cycles, args.workload)
                                {
                                    let request = Request {
                                        case,
                                        operation,
                                        rows: case.rows(args.rows),
                                        payload,
                                        count,
                                        seed: u64::from(sample)
                                            + 1
                                            + if args.growth_cycles {
                                                0
                                            } else {
                                                u64::from(round) * 7919
                                            },
                                        pattern,
                                    };
                                    // Admit the intended phase before executing either backend.
                                    let expected = expected(&mut model, request);
                                    let bytes =
                                        ic_wire::native::encode_request(RUN_ID, &request, LIMITS)?;
                                    let reply = if args.query {
                                        pic.query_call(
                                            canister,
                                            CanisterId::anonymous(),
                                            "benchmark_read",
                                            bytes,
                                        )
                                        .await
                                    } else {
                                        pic.update_call(
                                            canister,
                                            CanisterId::anonymous(),
                                            "benchmark",
                                            bytes,
                                        )
                                        .await
                                    }
                                    .map_err(|e| format!("{name} {operation:?}: {e:?}"))?;
                                    let actual: Response =
                                        ic_wire::native::decode_response(RUN_ID, &reply, LIMITS)?;
                                    let execution_instructions = if args.trace_execution_cost {
                                        let bytes = pic
                                            .query_call(
                                                canister,
                                                CanisterId::anonymous(),
                                                "benchmark_execution_cost",
                                                Vec::new(),
                                            )
                                            .await
                                            .map_err(|e| {
                                                format!("{name} execution counter: {e:?}")
                                            })?;
                                        Some(u64::from_le_bytes(
                                            bytes
                                                .as_slice()
                                                .try_into()
                                                .map_err(|_| "invalid execution counter size")?,
                                        ))
                                    } else {
                                        None
                                    };
                                    // A populated tree can exceed the bounded trace after
                                    // multiple lookups. Keep all ordinary operation samples,
                                    // but capture depth diagnostics only for single reads.
                                    let capture_read = (operation == Operation::Get
                                        && (!matches!(args.workload, Some(Workload::Depth))
                                            || count == 1))
                                        || (matches!(args.workload, Some(Workload::Bulk))
                                            && case.collection == Collection::Vector
                                            && count == 1);
                                    let read_trace = if args.trace_stable_reads && capture_read {
                                        let bytes = pic
                                            .query_call(
                                                canister,
                                                CanisterId::anonymous(),
                                                "stable_read_trace",
                                                Vec::new(),
                                            )
                                            .await
                                            .map_err(|e| format!("{name} read trace: {e:?}"))?;
                                        Some(decode_read_trace(&bytes)?)
                                    } else {
                                        None
                                    };
                                    let allocator_accesses =
                                        if args.trace_allocator_accesses && capture_read {
                                            let bytes = pic
                                                .query_call(
                                                    canister,
                                                    CanisterId::anonymous(),
                                                    "allocator_access_trace",
                                                    Vec::new(),
                                                )
                                                .await
                                                .map_err(|e| {
                                                    format!("{name} allocator access trace: {e:?}")
                                                })?;
                                            Some(decode_trace_rows(&bytes, 64)?)
                                        } else {
                                            None
                                        };
                                    writeln!(
                                        raw,
                                        "{}",
                                        serde_json::json!({"backend":name,"sample":sample,"round":round,"phase":phase,"request":request,"response":actual,"read_trace":read_trace,"allocator_accesses":allocator_accesses,"execution_instructions":execution_instructions})
                                    )?;
                                    raw.flush()?;
                                    assert_eq!(
                                        actual.decode.is_some(),
                                        case.measures_decode(operation)
                                    );
                                    if let Some(metrics) = actual.decode {
                                        let calls = if operation == Operation::MissAbove
                                            || operation == Operation::Control
                                        {
                                            0
                                        } else {
                                            u64::from(count)
                                        };
                                        assert_eq!(metrics.calls, calls);
                                        let encoded =
                                            SessionEnvelope::make(0, payload, false).encode()?;
                                        assert_eq!(metrics.bytes, calls * encoded.len() as u64);
                                        if calls != 0 {
                                            assert!(metrics.instructions > 0);
                                        }
                                        assert!(metrics.instructions <= actual.instructions);
                                        writeln!(
                                            stages,
                                            "{},{},{:?},{},{},{},{},{},{},{}",
                                            args.rows,
                                            payload,
                                            pattern,
                                            phase,
                                            name,
                                            round,
                                            actual.instructions,
                                            metrics.instructions,
                                            metrics.calls,
                                            metrics.bytes
                                        )?;
                                    }
                                    assert_eq!(
                                        actual.entries, expected,
                                        "{name} {operation:?}: result mismatch"
                                    );
                                    assert_eq!(actual.length, model.len() as u64);
                                    let key = (case, payload, count, pattern, phase, name.into());
                                    if let Some(instructions) = execution_instructions {
                                        execution_timings.entry(key.clone()).or_default().push(
                                            Measurement {
                                                round,
                                                instructions,
                                                stable_pages: actual.stable_pages,
                                            },
                                        );
                                    }
                                    timings.entry(key).or_default().push(Measurement {
                                        round,
                                        instructions: actual.instructions,
                                        stable_pages: actual.stable_pages,
                                    });
                                }
                            }
                            let expected_rows: Vec<_> = model.into_iter().collect();
                            let largest = expected_rows
                                .iter()
                                .map(|(_, bytes)| bytes.len())
                                .max()
                                .unwrap_or(0);
                            for chunk in expected_rows.chunks(transfer_rows(largest)) {
                                let request = Setup {
                                    start: chunk[0].0,
                                    count: chunk.len() as u32,
                                    payload: 0,
                                    ids: Vec::new(),
                                };
                                let reply = pic
                                    .query_call(
                                        canister,
                                        CanisterId::anonymous(),
                                        "verify",
                                        ic_wire::native::encode_request(3, &request, LIMITS)?,
                                    )
                                    .await
                                    .map_err(|e| format!("{name} final verification: {e:?}"))?;
                                let rows: Vec<(u64, Vec<u8>)> =
                                    ic_wire::native::decode_response(3, &reply, LIMITS)?;
                                assert_eq!(rows, chunk, "{name}: final map differs");
                            }
                            pic.stop_canister(canister, None).await.unwrap();
                            pic.delete_canister(canister, None).await.unwrap();
                        }
                    }
                }
            }
        }
    }
    pic.drop().await?;
    write_summary(
        fs::File::create(args.output.join("summary.csv"))?,
        &timings,
        args.rows,
        args.upstream_bucket_pages,
        args.rounds,
        false,
    )?;
    write_summary(
        fs::File::create(args.output.join("round-summary.csv"))?,
        &timings,
        args.rows,
        args.upstream_bucket_pages,
        args.rounds,
        true,
    )?;
    if args.trace_execution_cost {
        for (file, per_round) in [
            ("execution-summary.csv", false),
            ("execution-round-summary.csv", true),
        ] {
            write_summary(
                fs::File::create(args.output.join(file))?,
                &execution_timings,
                args.rows,
                args.upstream_bucket_pages,
                args.rounds,
                per_round,
            )?;
        }
    }
    println!(
        "Validated comparison: {}",
        args.output.join("summary.csv").display()
    );
    Ok(())
}
fn write_summary(
    mut summary: impl Write,
    timings: &Samples,
    rows: u32,
    bucket_pages: std::num::NonZeroU16,
    rounds: u32,
    per_round: bool,
) -> std::io::Result<()> {
    writeln!(
        summary,
        "rows,structure,case,key_type,value_type,logical_value_bytes,batch_or_scan_limit,pattern,operation,storage_median_instructions,dfinity_median_instructions,storage_over_dfinity,storage_median_stable_pages,dfinity_median_stable_pages,upstream_bucket_pages,payload_scope{}",
        if per_round { ",round" } else { "" }
    )?;
    for ((case, payload, count, pattern, operation, backend), samples) in timings {
        if backend != "ic-storage" {
            continue;
        }
        let other = timings.get(&(
            *case,
            *payload,
            *count,
            *pattern,
            operation.clone(),
            "ic-stable-structures".into(),
        ));
        for selected_round in if per_round {
            (0..rounds).map(Some).collect()
        } else {
            vec![None]
        } {
            let samples: Vec<_> = samples
                .iter()
                .filter(|s| selected_round.is_none_or(|round| s.round == round))
                .collect();
            let other: Vec<_> = other
                .into_iter()
                .flatten()
                .filter(|s| selected_round.is_none_or(|round| s.round == round))
                .collect();
            let a = median(&samples.iter().map(|s| s.instructions).collect::<Vec<_>>());
            let b = (!other.is_empty())
                .then(|| median(&other.iter().map(|s| s.instructions).collect::<Vec<_>>()));
            let a_pages = median(&samples.iter().map(|s| s.stable_pages).collect::<Vec<_>>());
            let b_pages = (!other.is_empty())
                .then(|| median(&other.iter().map(|s| s.stable_pages).collect::<Vec<_>>()));
            writeln!(
                summary,
                "{},{},{},{},{},{payload},{count},{pattern:?},{operation},{a},{},{},{a_pages},{},{},{}{}",
                case.rows(rows),
                case.structure(),
                csv_field(&case.label()),
                csv_field(case.key_type()),
                csv_field(case.value_type()),
                b.map_or_else(String::new, |n| n.to_string()),
                b.map_or_else(String::new, |n| format!("{:.4}", a as f64 / n as f64)),
                b_pages.map_or_else(String::new, |n| n.to_string()),
                bucket_pages,
                case.value.payload_scope(),
                selected_round.map_or_else(String::new, |round| format!(",{round}"))
            )?;
        }
    }
    Ok(())
}

fn median(values: &[u64]) -> u64 {
    let mut values = values.to_vec();
    values.sort_unstable();
    values[values.len() / 2]
}

fn install_encoding(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        "gzip"
    } else {
        "raw"
    }
}
fn module_size(bytes: &[u8]) -> usize {
    if install_encoding(bytes) == "gzip" {
        // Gzip's ISIZE is exact for these sub-4-GiB modules. The IC validates
        // the compressed module; this field is reporting metadata only.
        u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap()) as usize
    } else {
        bytes.len()
    }
}
fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}
fn case_sources(case: Case) -> (&'static str, &'static str) {
    (case.key.source(), case.value.source())
}
async fn probe_refusal(
    pic: &ic_testkit::pocket_ic::PocketIc,
    wasm: &[u8],
    case: Case,
    bucket: std::num::NonZeroU16,
    phase: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let canister = pic.create_canister().await?;
    pic.add_cycles(canister, 100_000_000_000_000).await?;
    let settings = Initialization {
        case,
        upstream_cache_slots: 0,
        upstream_bucket_pages: bucket,
        cold_cache: false,
        vector_open: false,
    };
    let installed = pic
        .install_canister(
            canister,
            wasm.to_vec(),
            ic_wire::native::encode_request(0, &settings, LIMITS)?,
            None,
        )
        .await;
    let error = if phase == "init" {
        format!(
            "{:?}",
            installed.expect_err("unbounded Vector unexpectedly initialized")
        )
    } else {
        installed?;
        format!(
            "{:?}",
            pic.update_call(
                canister,
                CanisterId::anonymous(),
                "setup",
                ic_wire::native::encode_request(
                    SETUP_ID,
                    &Setup {
                        start: 0,
                        count: 1,
                        payload: case.payloads()[0],
                        ids: vec![0]
                    },
                    LIMITS
                )?
            )
            .await
            .expect_err("native ICSS fixed Option unexpectedly accepted None")
        )
    };
    if phase == "init" {
        assert!(
            error.contains("unbounded"),
            "unexpected init refusal: {error}"
        );
    } else {
        assert!(
            error.contains("fixed-size"),
            "unexpected setup refusal: {error}"
        );
    }
    pic.stop_canister(canister, None).await?;
    pic.delete_canister(canister, None).await?;
    Ok(error)
}
fn combine_collections(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let directories: Vec<_> = Collection::ALL
        .iter()
        .map(|c| root.join(c.name().to_ascii_lowercase()))
        .collect();
    if !directories.iter().all(|p| p.join("summary.csv").is_file()) {
        return Ok(());
    }
    for filename in [
        "summary.csv",
        "round-summary.csv",
        "coverage.csv",
        "lookup-stages.csv",
    ] {
        let mut output = fs::File::create(root.join(filename))?;
        let mut header = None;
        for directory in &directories {
            let text = fs::read_to_string(directory.join(filename))?;
            let mut lines = text.lines();
            let first = lines.next().ok_or("empty subreport")?;
            if let Some(previous) = &header {
                assert_eq!(previous, first);
            } else {
                writeln!(output, "{first}")?;
                header = Some(first.to_owned());
            }
            for line in lines {
                writeln!(output, "{line}")?;
            }
        }
    }
    for filename in ["samples.jsonl", "refusals.jsonl"] {
        let mut output = fs::File::create(root.join(filename))?;
        for directory in &directories {
            std::io::copy(&mut fs::File::open(directory.join(filename))?, &mut output)?;
        }
    }
    let configurations: Vec<serde_json::Value> = directories
        .iter()
        .map(|p| {
            Ok(serde_json::from_slice(&fs::read(
                p.join("configuration.json"),
            )?)?)
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    fs::write(
        root.join("configuration.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"workload":"types","types":catalogue::DATA_TYPES.len(),"cases":MATRIX_CASES.len(),"collections":configurations,"status":"complete"}),
        )?,
    )?;
    println!(
        "Validated complete type matrix: {}",
        root.join("summary.csv").display()
    );
    Ok(())
}
fn population(rows: u32, shuffled: bool, mut state: u64) -> Vec<u64> {
    let mut ids: Vec<_> = (0..u64::from(rows)).collect();
    if shuffled {
        for end in (1..ids.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let slot = usize::try_from(state % (end as u64 + 1)).unwrap();
            ids.swap(end, slot);
        }
    }
    ids
}
fn expected(model: &mut BTreeMap<u64, Vec<u8>>, r: Request) -> Vec<(u64, Option<Vec<u8>>)> {
    use Operation::*;
    admit_phase(model, r);
    let ids = keys(r);
    let structure = r.case.structure();
    if r.operation == ValuesScan {
        return model
            .values()
            .take(r.count as usize)
            .enumerate()
            .map(|(rank, value)| (rank as u64, Some(value.clone())))
            .collect();
    }
    if matches!(r.operation, Nth | NthBack) {
        return ids
            .into_iter()
            .filter_map(|index| {
                let entry = if r.operation == Nth {
                    model.iter().nth(index as usize)
                } else {
                    model.iter().nth_back(index as usize)
                };
                entry.map(|(&key, value)| (key, Some(value.clone())))
            })
            .collect();
    }
    if r.operation == AlternatingScan {
        let mut rows: std::collections::VecDeque<_> = model
            .iter()
            .map(|(&key, value)| (key, Some(value.clone())))
            .collect();
        return (0..r.count)
            .map_while(|step| {
                if step % 2 == 0 {
                    rows.pop_front()
                } else {
                    rows.pop_back()
                }
            })
            .collect();
    }
    if r.operation == ScanSet {
        return model
            .iter_mut()
            .take(r.count as usize)
            .enumerate()
            .map(|(index, (&key, value))| {
                if index != 0 {
                    *value = r.case.expected_value(key, r.payload, true);
                }
                (key, Some(value.clone()))
            })
            .collect();
    }
    if matches!(r.operation, First | Last | IterLast | KeysLast | ValuesLast) {
        let entry = if r.operation == First {
            model.first_key_value()
        } else {
            model.last_key_value()
        };
        return entry
            .map(|(&key, value)| {
                (0..r.count)
                    .map(|rank| {
                        let key = if r.operation == ValuesLast {
                            u64::from(rank)
                        } else {
                            key
                        };
                        let value = if r.operation == KeysLast || structure == "Set" {
                            vec![]
                        } else {
                            value.clone()
                        };
                        (key, Some(value))
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    if matches!(
        r.operation,
        Scan | ReverseScan | KeysScan | BoundedScan | ReverseKeysScan | BoundedKeysScan
    ) {
        let all = structure == "Set"
            || structure == "Vector"
            || (matches!(r.operation, KeysScan | ReverseKeysScan) && structure == "Map");
        let rows: Vec<_> = model
            .iter()
            .filter(|(k, _)| {
                all || if r.operation == ReverseScan {
                    **k <= ids[0]
                } else {
                    **k >= ids[0]
                        && (!matches!(r.operation, BoundedScan | BoundedKeysScan)
                            || **k < ids[0] + u64::from(r.count.div_ceil(2)))
                }
            })
            .map(|(&k, v)| {
                (
                    k,
                    Some(
                        if matches!(r.operation, KeysScan | ReverseKeysScan | BoundedKeysScan)
                            || structure == "Set"
                        {
                            vec![]
                        } else {
                            v.clone()
                        },
                    ),
                )
            })
            .collect();
        return if matches!(r.operation, ReverseScan | ReverseKeysScan) {
            rows.into_iter().rev().take(r.count as usize).collect()
        } else {
            rows.into_iter().take(r.count as usize).collect()
        };
    }
    ids.into_iter()
        .map(|id| {
            let key = if structure == "Cell" {
                0
            } else if r.operation == Push {
                model.len() as u64
            } else if r.operation == Pop {
                model.len().saturating_sub(1) as u64
            } else {
                id
            };
            let result = match r.operation {
                Control => None,
                Get | WarmGet | Decode | MissAbove => {
                    if structure == "Set" {
                        Some(vec![u8::from(model.contains_key(&key))])
                    } else {
                        model.get(&key).cloned()
                    }
                }
                Contains => Some(vec![u8::from(model.contains_key(&key))]),
                Replace | Insert | Reinsert | Grow | Shrink => {
                    let old = model.insert(
                        key,
                        r.case
                            .expected_value(key, payload_for(r), replacement(r.operation)),
                    );
                    if structure == "Set" {
                        Some(vec![u8::from(old.is_none())])
                    } else {
                        old
                    }
                }
                Remove | RemoveMiss | RemoveInserted | Pop => {
                    let old = model.remove(&key);
                    if structure == "Set" {
                        Some(vec![u8::from(old.is_some())])
                    } else {
                        old
                    }
                }
                Set => {
                    model.insert(key, r.case.expected_value(key, r.payload, true));
                    None
                }
                Push => {
                    model.insert(key, r.case.expected_value(key, r.payload, false));
                    None
                }
                _ => unreachable!(),
            };
            (key, result)
        })
        .collect()
}
// Preconditions apply at phase entry. Repeated hot keys can legitimately turn
// later removals/reinsertions in the same batch into misses/replacements.
fn admit_phase(model: &BTreeMap<u64, Vec<u8>>, r: Request) {
    use Operation::*;
    for key in keys(r) {
        match r.operation {
            MissAbove | RemoveMiss | Insert | Reinsert => {
                assert!(
                    !model.contains_key(&key),
                    "{:?}: key {key} must be absent",
                    r.operation
                );
            }
            Get | WarmGet | Decode | Contains | Replace | Set | Remove | RemoveInserted | Grow
            | Shrink => {
                let old = model
                    .get(&key)
                    .unwrap_or_else(|| panic!("{:?}: key {key} must exist", r.operation));
                if matches!(r.operation, Grow | Shrink) {
                    let previous_payload = if r.operation == Grow {
                        r.payload
                    } else {
                        payload_for(Request {
                            operation: Grow,
                            ..r
                        })
                    };
                    assert_eq!(
                        *old,
                        r.case.expected_value(key, previous_payload, true),
                        "{:?}: unexpected previous value",
                        r.operation
                    );
                }
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    // The library owns tests.rs; keep binary test bodies in the shared tests directory.
    use super::*;
    mod driver;
}
