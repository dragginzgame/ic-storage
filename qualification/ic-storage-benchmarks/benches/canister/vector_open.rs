//! Reopen collections without rebuilding the allocation owner for each request.
use super::*;

struct MemorySource {
    #[cfg(feature = "owned")]
    runtime: ic_storage::MemoryRuntime<ic_storage::DefaultMemoryImpl>,
    #[cfg(feature = "owned")]
    key: ic_storage::StableKey,
    #[cfg(not(feature = "owned"))]
    memory: BenchMemory,
}
impl MemorySource {
    fn new(pages: std::num::NonZeroU16) -> Self {
        #[cfg(feature = "owned")]
        {
            let _ = pages;
            Self {
                runtime: new_runtime(),
                key: ic_storage::StableKey::parse("benchmark.rows.v1").unwrap(),
            }
        }
        #[cfg(not(feature = "owned"))]
        {
            Self {
                memory: new_memory(pages),
            }
        }
    }
    fn open(&self) -> BenchMemory {
        #[cfg(feature = "owned")]
        {
            self.runtime.open_memory(&self.key).unwrap()
        }
        #[cfg(not(feature = "owned"))]
        {
            self.memory.clone()
        }
    }
}
struct ReopenedVector<V: BenchValue + Storable> {
    source: MemorySource,
    case: Case,
    marker: std::marker::PhantomData<V>,
}
impl<V: BenchValue + Storable> ReopenedVector<V> {
    fn new(settings: Initialization) -> Self {
        let source = MemorySource::new(settings.upstream_bucket_pages);
        #[cfg(feature = "owned")]
        drop(ic_storage::adapter::StableVec::<V>::create(source.open()).unwrap());
        #[cfg(not(feature = "owned"))]
        drop(upstream::Vec::<V, _>::new(source.open()));
        Self {
            source,
            case: settings.case,
            marker: std::marker::PhantomData,
        }
    }
    fn open(&self) -> TypedVector<V> {
        #[cfg(feature = "owned")]
        let vector = ic_storage::adapter::StableVec::load(self.source.open()).unwrap();
        #[cfg(not(feature = "owned"))]
        let vector = upstream::Vec::init(self.source.open());
        TypedVector {
            case: self.case,
            vector,
        }
    }
}
impl<V: BenchValue + Storable> Benchmark for ReopenedVector<V> {
    fn setup(&mut self, r: Setup) -> u64 {
        self.open().setup(r)
    }
    fn verify(&self, r: Setup) -> Vec<(u64, Vec<u8>)> {
        self.open().verify(r)
    }
    fn run(&mut self, r: Request) -> Response {
        let start = ic0::performance_counter(0);
        let mut vector = self.open();
        let mut response = vector.run(r);
        drop(vector);
        response.instructions = ic0::performance_counter(0) - start;
        response
    }
}
pub(super) fn new(settings: Initialization) -> Box<dyn Benchmark> {
    match settings.case {
        ic_storage_benchmarks::case!(Vector, U64, U64) => {
            Box::new(ReopenedVector::<u64>::new(settings))
        }
        ic_storage_benchmarks::case!(Vector, U64, Array32) => {
            Box::new(ReopenedVector::<[u8; 32]>::new(settings))
        }
        ic_storage_benchmarks::case!(Vector, U64, Bounded255) => {
            Box::new(ReopenedVector::<BoundedBytes<255>>::new(settings))
        }
        ic_storage_benchmarks::case!(Vector, U64, Bounded256) => {
            Box::new(ReopenedVector::<BoundedBytes<256>>::new(settings))
        }
        ic_storage_benchmarks::case!(Vector, U64, Chunk) => {
            Box::new(ReopenedVector::<ChunkBytes>::new(settings))
        }
        ic_storage_benchmarks::case!(Vector, U64, WireRecord) => {
            Box::new(ReopenedVector::<WireRecord>::new(settings))
        }
        _ => panic!("unsupported vector-open type"),
    }
}
