//! Real Metrics adapter operations; no duplicate checkpoint codec or save policy.
use ic_metrics::MeasurementHistogram as Histogram;
use ic_metrics_storage::{MetricIdentity, MetricKey, MetricsStore, StoreError};
use ic_storage::{
    AllocationGrants, GenericAllocationPolicy, MemoryAuthority, MemoryBacking, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey, adapter::StableBTreeMap,
};

pub const OPERATIONS: [&str; 7] = [
    "changed",
    "unchanged",
    "regression",
    "load",
    "remove",
    "missing",
    "insert",
];

pub struct Workload<M: MemoryBacking, const N: usize> {
    store: MetricsStore<M>,
    current: Histogram<N>,
    empty: Histogram<N>,
    key: MetricKey,
    identity: MetricIdentity,
}

impl<M: MemoryBacking, const N: usize> Workload<M, N> {
    pub fn new(memory: M, rows: usize) -> Self {
        let mut runtime = MemoryRuntime::new(memory).unwrap();
        let declarations = SealedDeclarationSnapshot::new(vec![
            MemoryRequest::new("bench", "bench.metrics.v1").unwrap(),
        ])
        .unwrap();
        let grants =
            AllocationGrants::new(vec![MemoryAuthority::new("bench", "bench.").unwrap()]).unwrap();
        runtime
            .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
            .unwrap();
        let memory = runtime
            .open_memory(&StableKey::parse("bench.metrics.v1").unwrap())
            .unwrap();
        let mut store = MetricsStore::new(StableBTreeMap::create(memory).unwrap());
        let bounds = std::array::from_fn(|i| (i as u64 + 1) * 1000);
        let empty = Histogram::new(bounds).unwrap();
        let mut current = empty;
        for sample in 0..=N {
            current.record(sample as u64 * 1000);
        }
        let identity = MetricIdentity::new("instructions", 7).unwrap();
        for i in 0..rows {
            let key = MetricKey::new("benchmark", &format!("operation-{i:04}")).unwrap();
            store.save_histogram(&key, &identity, &current).unwrap();
        }
        Self {
            store,
            current,
            empty,
            key: MetricKey::new("benchmark", &format!("operation-{:04}", rows / 2)).unwrap(),
            identity,
        }
    }

    // Record generation and assertions are outside the measured adapter call.
    // Every run preserves state for the next operation/round.
    pub fn step(&mut self, operation: usize, mut clock: impl FnMut() -> u64) -> u64 {
        if operation == 0 {
            self.current.record(u64::MAX / 1024);
        }
        let start = clock();
        let end = match operation {
            0 | 1 | 6 => {
                let result = self
                    .store
                    .save_histogram(&self.key, &self.identity, &self.current);
                let end = clock();
                result.unwrap();
                end
            }
            2 => {
                let result = self
                    .store
                    .save_histogram(&self.key, &self.identity, &self.empty);
                let end = clock();
                assert_eq!(result, Err(StoreError::StateRegression));
                end
            }
            3 => {
                let result = self.store.load_histogram(
                    &self.key,
                    &self.identity,
                    self.current.upper_bounds(),
                );
                let end = clock();
                assert_eq!(result.unwrap(), Some(self.current));
                end
            }
            4 | 5 => {
                let result = self.store.remove(&self.key, &self.identity);
                let end = clock();
                assert_eq!(result.unwrap(), operation == 4);
                end
            }
            _ => panic!("unknown checkpoint operation"),
        };
        end - start
    }
}
