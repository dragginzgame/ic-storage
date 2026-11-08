//! Identical workload compiled separately for each backend.
#[cfg(target_arch = "wasm32")]
mod canister {
    use ic_platform_runtime::ic0;
    use ic_storage_benchmarks::*;
    use serde::{Serialize, de::DeserializeOwned};
    use std::cell::{Cell, RefCell};

    #[cfg(any(
        all(feature = "owned", any(feature = "dfinity", feature = "dfinity-head")),
        all(feature = "dfinity", feature = "dfinity-head"),
        not(any(feature = "owned", feature = "dfinity", feature = "dfinity-head"))
    ))]
    compile_error!("select exactly one backend: owned or dfinity");

    #[cfg(feature = "owned")]
    type Map<K, V> = ic_storage::adapter::StableBTreeMap<K, V>;
    #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
    type Map<K, V> = ic_storage_benchmarks::upstream::BTreeMap<K, V, BenchMemory>;
    #[cfg(feature = "owned")]
    use ic_storage::adapter::Storable;
    #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
    use ic_storage_benchmarks::upstream::Storable;
    thread_local! { static MAP: RefCell<Option<Box<dyn Benchmark>>> = const { RefCell::new(None) }; }
    thread_local! { static INITIALIZATION_COST: Cell<u64> = const { Cell::new(0) }; }

    // A failed Storage operation fails the benchmark; upstream reports failures
    // through traps. Successful-path admission stays inside the timed operation.
    macro_rules! checked {
        ($operation:expr) => {{
            #[cfg(feature = "owned")]
            {
                $operation.expect("benchmark Storage operation")
            }
            #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
            {
                $operation
            }
        }};
    }

    struct TypedMap<K: BenchKey + Storable, V: BenchValue + Storable> {
        case: Case,
        map: Map<K, V>,
        settings: Initialization,
    }
    trait Benchmark {
        fn setup(&mut self, request: Setup) -> u64;
        fn verify(&self, request: Setup) -> Vec<(u64, Vec<u8>)>;
        fn run(&mut self, request: Request) -> Response;
    }

    fn argument<T: DeserializeOwned>(id: u64) -> T {
        let size = ic0::msg_arg_data_size();
        assert!(size <= LIMITS.bytes);
        let mut bytes = vec![0; size];
        ic0::msg_arg_data_copy(&mut bytes, 0);
        ic_wire::native::decode_request(id, &bytes, LIMITS).unwrap()
    }
    fn reply(id: u64, result: &impl Serialize) {
        let bytes = ic_wire::native::encode_response(id, result, LIMITS).unwrap_or_else(|error| {
            ic0::trap(format!("benchmark response encoding: {error}").as_bytes())
        });
        ic0::msg_reply_data_append(&bytes);
        ic0::msg_reply();
    }
    #[cfg(feature = "owned")]
    type BenchMemory = ic_storage::RuntimeMemory<ic_storage::DefaultMemoryImpl>;
    #[cfg(all(
        any(feature = "dfinity", feature = "dfinity-head"),
        not(feature = "dfinity-governed")
    ))]
    type BenchMemory = ic_storage_benchmarks::upstream::memory_manager::VirtualMemory<
        ic_storage_benchmarks::upstream::DefaultMemoryImpl,
    >;
    #[cfg(feature = "dfinity-governed")]
    #[derive(Clone)]
    struct BenchMemory(std::rc::Rc<ic_storage::RuntimeMemory<ic_storage::DefaultMemoryImpl>>);
    #[cfg(feature = "dfinity-governed")]
    impl ic_storage_benchmarks::upstream::Memory for BenchMemory {
        fn size(&self) -> u64 {
            self.0.size()
        }
        fn grow(&self, pages: u64) -> i64 {
            ic_storage::Memory::grow(self.0.as_ref(), pages)
        }
        fn read(&self, offset: u64, bytes: &mut [u8]) {
            self.0.read(offset, bytes);
        }
        unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
            // SAFETY: ICSS and Storage require the same exclusive destination
            // and guarantee complete initialization on successful return.
            unsafe { ic_storage::Memory::read_unsafe(self.0.as_ref(), offset, dst, count) }
        }
        fn write(&self, offset: u64, bytes: &[u8]) {
            self.0.write(offset, bytes);
        }
    }
    #[cfg(any(feature = "owned", feature = "dfinity-governed"))]
    fn new_runtime() -> ic_storage::MemoryRuntime<ic_storage::DefaultMemoryImpl> {
        use ic_storage::*;
        let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
        let requests = SealedDeclarationSnapshot::new(vec![
            MemoryRequest::new("benchmark", "benchmark.rows.v1").unwrap(),
        ])
        .unwrap();
        let grants = AllocationGrants::new(vec![
            MemoryAuthority::new("benchmark", "benchmark.").unwrap(),
        ])
        .unwrap();
        runtime
            .bootstrap(&requests, &grants, &GenericAllocationPolicy)
            .unwrap();
        runtime
    }
    fn new_memory(bucket_pages: std::num::NonZeroU16) -> BenchMemory {
        #[cfg(any(feature = "owned", feature = "dfinity-governed"))]
        {
            let _ = bucket_pages;
            let runtime = new_runtime();
            let memory = runtime
                .open_memory(&ic_storage::StableKey::parse("benchmark.rows.v1").unwrap())
                .unwrap();
            #[cfg(feature = "owned")]
            {
                memory
            }
            #[cfg(feature = "dfinity-governed")]
            {
                BenchMemory(std::rc::Rc::new(memory))
            }
        }
        #[cfg(all(
            any(feature = "dfinity", feature = "dfinity-head"),
            not(feature = "dfinity-governed")
        ))]
        {
            use ic_storage_benchmarks::upstream::{
                DefaultMemoryImpl,
                memory_manager::{MemoryId, MemoryManager},
            };
            MemoryManager::init_with_bucket_size(DefaultMemoryImpl::default(), bucket_pages.get())
                .get(MemoryId::new(0))
        }
    }
    fn new_map<K: BenchKey + Storable, V: BenchValue + Storable>(
        settings: Initialization,
    ) -> Map<K, V> {
        #[cfg(feature = "owned")]
        {
            Map::create(new_memory(settings.upstream_bucket_pages)).unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        {
            Map::init(new_memory(settings.upstream_bucket_pages))
        }
    }
    fn configure_cache<K: BenchKey + Storable, V: BenchValue + Storable>(
        map: &mut Map<K, V>,
        settings: Initialization,
    ) {
        #[cfg(feature = "owned")]
        map.set_node_cache_budget(settings.cache_bytes);
        #[cfg(feature = "dfinity-head")]
        map.node_cache_resize(settings.upstream_cache_slots);
        #[cfg(feature = "dfinity")]
        let _ = (map, settings);
    }
    fn configured_map<K: BenchKey + Storable, V: BenchValue + Storable>(
        settings: Initialization,
    ) -> Map<K, V> {
        let mut map = new_map(settings);
        configure_cache(&mut map, settings);
        map
    }
    mod collections;
    #[cfg(not(feature = "footprint"))]
    mod matrix;
    #[cfg(not(feature = "footprint"))]
    mod vector_open;
    use collections::{Region, TypedCell, TypedSet, TypedVector};
    #[unsafe(export_name = "canister_init")]
    extern "C" fn init() {
        let start = ic0::performance_counter(0);
        initialize();
        INITIALIZATION_COST.with(|cost| cost.set(ic0::performance_counter(0) - start));
    }
    fn initialize() {
        // Refusal probes must identify the violated bound, rather than treating
        // every unreachable trap as the expected incompatibility. The hook is
        // outside measured operations and only runs on failure.
        std::panic::set_hook(Box::new(|info| ic0::trap(info.to_string().as_bytes())));
        let settings: Initialization = argument(0);
        let case = settings.case;
        #[cfg(not(feature = "footprint"))]
        if settings.vector_open {
            MAP.with(|slot| *slot.borrow_mut() = Some(vector_open::new(settings)));
            return;
        }
        macro_rules! typed {
            ($key:ty, $value:ty) => {
                Box::new(TypedMap::<$key, $value> {
                    case,
                    map: configured_map(settings),
                    settings,
                }) as Box<dyn Benchmark>
            };
        }
        // A matched small-type control for Wasm footprint attribution. Keep the
        // full catalogue as the default; both suppliers use these same cases.
        #[cfg(feature = "footprint")]
        let map: Box<dyn Benchmark> = {
            assert!(
                !settings.vector_open,
                "footprint build excludes vector-open"
            );
            match case {
                Case::U64U64 => typed!(u64, u64),
                Case::U64Bytes => typed!(u64, Vec<u8>),
                Case::SetU64 => {
                    Box::new(TypedSet::<u64>::new(case, settings.upstream_bucket_pages))
                }
                Case::VectorU64 => Box::new(TypedVector::<u64>::new(
                    case,
                    settings.upstream_bucket_pages,
                )),
                Case::CellFixed32 => Box::new(TypedCell::<[u8; 32]>::new(
                    case,
                    settings.upstream_bucket_pages,
                )),
                Case::Typed {
                    collection: Collection::Region,
                    key: DataType::U64,
                    value: DataType::U64,
                } => Box::new(Region::<u64, u64>::new(
                    case,
                    settings.upstream_bucket_pages,
                )),
                _ => panic!("footprint build supports only depth workload cases"),
            }
        };
        #[cfg(not(feature = "footprint"))]
        let map = match case {
            Case::Typed {
                collection,
                key,
                value,
            } => matrix::create(case, collection, key, value, settings),
            Case::U32U32 => typed!(u32, u32),
            Case::U64Unit => typed!(u64, ()),
            Case::U64BoundedBytes => typed!(u64, BoundedBytes<255>),
            Case::BoundedBytesU64 => typed!(BoundedBytes<255>, u64),
            Case::BoundedBytesBoundedBytes => typed!(BoundedBytes<255>, BoundedBytes<255>),
            Case::BoundedBytesBytes => typed!(BoundedBytes<255>, Vec<u8>),
            Case::BytesU64 => typed!(Vec<u8>, u64),
            Case::BytesBoundedBytes => typed!(Vec<u8>, BoundedBytes<255>),
            Case::BytesBytes => typed!(Vec<u8>, Vec<u8>),
            Case::TupleOption => typed!(TupleKey, OptionalU64),
            Case::SetBytes => Box::new(TypedSet::<Vec<u8>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::SetBoundedBytes => Box::new(TypedSet::<BoundedBytes<255>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::SetTuple => Box::new(TypedSet::<TupleKey>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorUnit => {
                Box::new(TypedVector::<()>::new(case, settings.upstream_bucket_pages))
            }
            Case::VectorU8 => {
                Box::new(TypedVector::<u8>::new(case, settings.upstream_bucket_pages))
            }
            Case::VectorU16 => Box::new(TypedVector::<u16>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorU32 => Box::new(TypedVector::<u32>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBool => Box::new(TypedVector::<bool>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorF32 => Box::new(TypedVector::<f32>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorOption => Box::new(TypedVector::<OptionalU64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBounded255 => Box::new(TypedVector::<BoundedBytes<255>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBounded256 => Box::new(TypedVector::<BoundedBytes<256>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBounded65535 => Box::new(TypedVector::<BoundedBytes<65535>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBounded65536 => Box::new(TypedVector::<BoundedBytes<65536>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::CellString => Box::new(TypedCell::<String>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::CellUnit => Box::new(TypedCell::<()>::new(case, settings.upstream_bucket_pages)),
            Case::CellBool => {
                Box::new(TypedCell::<bool>::new(case, settings.upstream_bucket_pages))
            }
            Case::CellOption => Box::new(TypedCell::<OptionalU64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::U64Bytes => typed!(u64, Vec<u8>),
            Case::U64U64 => typed!(u64, u64),
            Case::U64U128 => typed!(u64, u128),
            Case::Bytes32Bytes32 => typed!([u8; 32], [u8; 32]),
            Case::StringString => typed!(String, String),
            Case::U64Record => typed!(u64, WireRecord),
            Case::SetU64 => Box::new(TypedSet::<u64>::new(case, settings.upstream_bucket_pages)),
            Case::SetString => Box::new(TypedSet::<String>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorU64 => Box::new(TypedVector::<u64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBytes32 => Box::new(TypedVector::<[u8; 32]>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorBytes => Box::new(TypedVector::<ChunkBytes>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorSession => Box::new(TypedVector::<SessionEnvelope>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorRecord => Box::new(TypedVector::<WireRecord>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::CellBytes => Box::new(TypedCell::<Vec<u8>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::CellRecord => Box::new(TypedCell::<WireRecord>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::RegionBytes => Box::new(Region::<u64, Vec<u8>>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::I64I64 => typed!(Signed64, Signed64),
            Case::U128U128 => typed!(u128, u128),
            Case::Bytes64U128 => typed!([u8; 64], u128),
            Case::U64Fixed31 => typed!(u64, [u8; 31]),
            Case::U64Fixed32 => typed!(u64, [u8; 32]),
            Case::U64Fixed33 => typed!(u64, [u8; 33]),
            Case::U64Fixed256 => typed!(u64, [u8; 256]),
            Case::SetI64 => Box::new(TypedSet::<Signed64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::SetBytes64 => Box::new(TypedSet::<[u8; 64]>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorI64 => Box::new(TypedVector::<Signed64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorU128 => Box::new(TypedVector::<u128>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::VectorF64 => Box::new(TypedVector::<f64>::new(
                case,
                settings.upstream_bucket_pages,
            )),
            Case::CellU128 => {
                Box::new(TypedCell::<u128>::new(case, settings.upstream_bucket_pages))
            }
            Case::CellFixed32 => Box::new(TypedCell::<[u8; 32]>::new(
                case,
                settings.upstream_bucket_pages,
            )),
        };
        MAP.with(|slot| *slot.borrow_mut() = Some(map));
    }
    #[unsafe(export_name = "canister_query initialization_cost")]
    extern "C" fn initialization_cost() {
        argument::<()>(INIT_COST_ID);
        INITIALIZATION_COST.with(|cost| reply(INIT_COST_ID, &cost.get()));
    }
    #[unsafe(export_name = "canister_update setup")]
    extern "C" fn setup() {
        let r: Setup = argument(SETUP_ID);
        assert!(r.count <= 128 && r.start + u64::from(r.count) <= u64::from(MAX_ROWS));
        assert!(r.ids.iter().all(|&id| id < u64::from(MAX_ROWS)));
        MAP.with(|slot| reply(SETUP_ID, &slot.borrow_mut().as_mut().unwrap().setup(r)));
    }
    #[unsafe(export_name = "canister_query verify")]
    extern "C" fn verify() {
        let r: Setup = argument(3);
        assert!(r.count <= 128);
        MAP.with(|slot| reply(3, &slot.borrow().as_ref().unwrap().verify(r)));
    }
    #[unsafe(export_name = "canister_query benchmark_read")]
    extern "C" fn benchmark_read() {
        let r: Request = argument(RUN_ID);
        assert_eq!(r.case, Case::VectorSession);
        assert!((1..=max_batch(r.case)).contains(&r.count));
        assert!(matches!(
            r.operation,
            Operation::Control
                | Operation::Get
                | Operation::WarmGet
                | Operation::Decode
                | Operation::MissAbove
        ));
        MAP.with(|slot| reply(RUN_ID, &slot.borrow_mut().as_mut().unwrap().run(r)));
    }
    #[unsafe(export_name = "canister_update benchmark")]
    extern "C" fn benchmark() {
        let r: Request = argument(RUN_ID);
        assert!((1..=max_batch(r.case)).contains(&r.count));
        MAP.with(|slot| reply(RUN_ID, &slot.borrow_mut().as_mut().unwrap().run(r)));
    }
    impl<K: BenchKey + Storable, V: BenchValue + Storable> Benchmark for TypedMap<K, V> {
        fn setup(&mut self, r: Setup) -> u64 {
            assert!(self.case.accepts_payload(r.payload));
            for id in population_ids(&r) {
                assert!(
                    checked!(
                        self.map
                            .insert(K::from_id(id), V::make(id, r.payload, false))
                    )
                    .is_none()
                );
            }
            self.map.len()
        }
        fn verify(&self, r: Setup) -> Vec<(u64, Vec<u8>)> {
            self.map
                .range(K::from_id(r.start)..)
                .take(r.count as usize)
                .map(|entry| {
                    let (key, value) = checked!(checked!(entry).into_pair());
                    (key.id(), value.canonical())
                })
                .collect()
        }
        #[cfg_attr(
            feature = "dfinity-governed",
            expect(
                clippy::double_ended_iterator_last,
                reason = "Measure ICSS last() as requested, without substituting next_back()"
            )
        )]
        fn run(&mut self, r: Request) -> Response {
            if self.settings.cold_cache {
                configure_cache(&mut self.map, self.settings);
            }
            assert_eq!(r.case, self.case);
            assert!(self.case.accepts_payload(r.payload));
            let ids = keys(r);
            let skips: Vec<_> = if matches!(r.operation, Operation::Nth | Operation::NthBack) {
                ids.iter().map(|&id| id as usize).collect()
            } else {
                Vec::new()
            };
            let scan_start = K::from_id(ids[0]);
            let scan_end = matches!(
                r.operation,
                Operation::BoundedScan | Operation::BoundedKeysScan
            )
            .then(|| K::from_id(ids[0] + u64::from(r.count.div_ceil(2))));
            // Prepare owned input and result keys before timing: inserts consume keys.
            let inputs: Vec<_> = ids
                .into_iter()
                .map(|id| {
                    (
                        K::from_id(id),
                        K::from_id(id),
                        matches!(
                            r.operation,
                            Operation::Replace
                                | Operation::Insert
                                | Operation::Reinsert
                                | Operation::Grow
                                | Operation::Shrink
                        )
                        .then(|| V::make(id, payload_for(r), replacement(r.operation))),
                    )
                })
                .collect();
            let mut entries = Vec::with_capacity(r.count as usize);
            let mut booleans = Vec::with_capacity(r.count as usize);
            let mut key_entries = Vec::with_capacity(r.count as usize);
            let mut values = if matches!(r.operation, Operation::ValuesScan | Operation::ValuesLast)
            {
                Vec::with_capacity(r.count as usize)
            } else {
                Vec::new()
            };
            let start = ic0::performance_counter(0);
            if r.operation == Operation::IterLast {
                for _ in 0..r.count {
                    if let Some(entry) = self.map.iter().last() {
                        let (key, value) = checked!(checked!(entry).into_pair());
                        entries.push((key, Some(value)));
                    }
                }
            } else if r.operation == Operation::KeysLast {
                for _ in 0..r.count {
                    if let Some(key) = self.map.keys().last() {
                        key_entries.push(checked!(key));
                    }
                }
            } else if r.operation == Operation::ValuesLast {
                for _ in 0..r.count {
                    if let Some(value) = self.map.values().last() {
                        values.push(checked!(value));
                    }
                }
            } else if r.operation == Operation::KeysScan {
                for key in self.map.keys().take(r.count as usize) {
                    key_entries.push(checked!(key));
                }
            } else if r.operation == Operation::ReverseKeysScan {
                for key in self.map.keys().rev().take(r.count as usize) {
                    key_entries.push(checked!(key));
                }
            } else if r.operation == Operation::BoundedKeysScan {
                for key in self
                    .map
                    .keys_range(scan_start..scan_end.expect("bounded key scan endpoint"))
                    .take(r.count as usize)
                {
                    key_entries.push(checked!(key));
                }
            } else if r.operation == Operation::ValuesScan {
                for value in self.map.values().take(r.count as usize) {
                    values.push(checked!(value));
                }
            } else if matches!(r.operation, Operation::Nth | Operation::NthBack) {
                for skip in skips {
                    let entry = if r.operation == Operation::Nth {
                        self.map.iter().nth(skip)
                    } else {
                        self.map.iter().nth_back(skip)
                    };
                    let (key, value) =
                        checked!(checked!(entry.expect("admitted ordered index")).into_pair());
                    entries.push((key, Some(value)));
                }
            } else if r.operation == Operation::AlternatingScan {
                let mut iter = self.map.iter();
                for step in 0..r.count {
                    let entry = if step % 2 == 0 {
                        iter.next()
                    } else {
                        iter.next_back()
                    };
                    let Some(entry) = entry else {
                        break;
                    };
                    let (key, value) = checked!(checked!(entry).into_pair());
                    entries.push((key, Some(value)));
                }
            } else if matches!(r.operation, Operation::First | Operation::Last) {
                for _ in 0..r.count {
                    let entry = if r.operation == Operation::First {
                        checked!(self.map.first_key_value())
                    } else {
                        checked!(self.map.last_key_value())
                    };
                    if let Some((key, value)) = entry {
                        entries.push((key, Some(value)));
                    }
                }
            } else if r.operation == Operation::BoundedScan {
                for entry in self
                    .map
                    .range(scan_start..scan_end.expect("bounded scan endpoint"))
                    .take(r.count as usize)
                {
                    let (key, value) = checked!(checked!(entry).into_pair());
                    entries.push((key, Some(value)));
                }
            } else if r.operation == Operation::ReverseScan {
                for entry in self.map.range(..=scan_start).rev().take(r.count as usize) {
                    let (key, value) = checked!(checked!(entry).into_pair());
                    entries.push((key, Some(value)));
                }
            } else if r.operation == Operation::Scan {
                for entry in self.map.range(scan_start..).take(r.count as usize) {
                    let (key, value) = checked!(checked!(entry).into_pair());
                    entries.push((key, Some(value)));
                }
            } else {
                for (key, result_key, value) in inputs {
                    let result = match r.operation {
                        Operation::Control => {
                            std::hint::black_box(&key);
                            None
                        }
                        Operation::Get | Operation::MissAbove => checked!(self.map.get(&key)),
                        Operation::Replace
                        | Operation::Insert
                        | Operation::Reinsert
                        | Operation::Grow
                        | Operation::Shrink => {
                            checked!(self.map.insert(key, value.unwrap()))
                        }
                        Operation::Remove | Operation::RemoveMiss | Operation::RemoveInserted => {
                            checked!(self.map.remove(&key))
                        }
                        Operation::Contains => {
                            let found = checked!(self.map.contains_key(&key));
                            booleans.push((result_key, found));
                            continue;
                        }
                        _ => unreachable!(),
                    };
                    entries.push((result_key, result));
                }
            }
            let instructions = ic0::performance_counter(0) - start;
            Response {
                decode: None,
                instructions,
                // Normalization is deliberately after the measured interval.
                entries: if r.operation == Operation::Contains {
                    booleans
                        .into_iter()
                        .map(|(k, v)| (k.id(), Some(vec![u8::from(v)])))
                        .collect()
                } else if matches!(r.operation, Operation::ValuesScan | Operation::ValuesLast) {
                    values
                        .into_iter()
                        .enumerate()
                        .map(|(rank, value)| (rank as u64, Some(value.canonical())))
                        .collect()
                } else if matches!(
                    r.operation,
                    Operation::KeysScan
                        | Operation::ReverseKeysScan
                        | Operation::BoundedKeysScan
                        | Operation::KeysLast
                ) {
                    key_entries
                        .into_iter()
                        .map(|k| (k.id(), Some(vec![])))
                        .collect()
                } else {
                    entries
                        .into_iter()
                        .map(|(key, value)| (key.id(), value.map(V::canonical)))
                        .collect()
                },
                length: self.map.len(),
                stable_pages: ic0::stable64_size(),
            }
        }
    }
}
