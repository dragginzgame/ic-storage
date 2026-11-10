use ic_platform_runtime::ic0;
#[cfg(feature = "dfinity")]
use ic_stable_structures::{
    DefaultMemoryImpl, Memory,
    memory_manager::{MemoryId, MemoryManager},
};
#[cfg(feature = "owned")]
use ic_storage::{DefaultMemoryImpl, Memory, MemoryRuntime};
use std::cell::RefCell;
#[cfg(feature = "owned")]
type Runtime = MemoryRuntime<DefaultMemoryImpl>;
#[cfg(feature = "dfinity")]
type Runtime = MemoryManager<DefaultMemoryImpl>;
#[cfg(feature = "owned")]
type Handle = ic_storage::RuntimeMemory<DefaultMemoryImpl>;
#[cfg(feature = "dfinity")]
type Handle = ic_stable_structures::memory_manager::VirtualMemory<DefaultMemoryImpl>;

struct State {
    runtime: Option<Runtime>,
    held: Option<Handle>,
    count: u64,
    #[cfg(feature = "owned")]
    keys: Vec<ic_storage::StableKey>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
fn input<const N: usize>() -> [u8; N] {
    assert_eq!(ic0::msg_arg_data_size(), N);
    let mut bytes = [0; N];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    bytes
}
fn runtime() -> Runtime {
    #[cfg(feature = "owned")]
    {
        Runtime::new(DefaultMemoryImpl::default()).unwrap()
    }
    #[cfg(feature = "dfinity")]
    {
        Runtime::init_with_bucket_size(DefaultMemoryImpl::default(), 8)
    }
}
impl State {
    fn admit(&mut self) {
        #[cfg(feature = "owned")]
        {
            use ic_storage::*;
            let declarations = SealedDeclarationSnapshot::new(
                self.keys
                    .iter()
                    .map(|key| MemoryRequest::new("bench", key.as_str()).unwrap())
                    .collect(),
            )
            .unwrap();
            let grants =
                AllocationGrants::new(vec![MemoryAuthority::new("bench", "bench.").unwrap()])
                    .unwrap();
            self.runtime
                .as_mut()
                .unwrap()
                .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
                .unwrap();
        }
    }
    #[cfg(feature = "owned")]
    fn memory(&self, index: u64) -> ic_storage::RuntimeMemory<DefaultMemoryImpl> {
        self.runtime
            .as_ref()
            .unwrap()
            .open_memory(&self.keys[index as usize])
            .unwrap()
    }
    #[cfg(feature = "dfinity")]
    fn memory(
        &self,
        index: u64,
    ) -> ic_stable_structures::memory_manager::VirtualMemory<DefaultMemoryImpl> {
        self.runtime
            .as_ref()
            .unwrap()
            .get(MemoryId::new(index.try_into().unwrap()))
    }
}
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    let args = input::<16>();
    let count = u64::from_le_bytes(args[..8].try_into().unwrap());
    let buckets = u64::from_le_bytes(args[8..].try_into().unwrap());
    assert!((1..=1024).contains(&count) && buckets <= 32);
    #[cfg(feature = "dfinity")]
    assert!(count <= 255);
    let mut state = State {
        runtime: Some(runtime()),
        held: None,
        count,
        #[cfg(feature = "owned")]
        keys: (0..count)
            .map(|i| ic_storage::StableKey::parse(format!("bench.memory{i}.v1")).unwrap())
            .collect(),
    };
    state.admit();
    if buckets != 0 {
        let memory = state.memory(0);
        assert_eq!(Memory::grow(&memory, buckets * 8), 0);
        memory.write(0, &42_u64.to_le_bytes());
    }
    STATE.with(|slot| *slot.borrow_mut() = Some(state));
}
#[unsafe(export_name = "canister_update run")]
extern "C" fn run() {
    let operation = input::<1>()[0];
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.as_mut().unwrap();
        // Reopen drops the prior owner before timing. Cold reconstruction only;
        // Storage policy bootstrap follows the timer, not part of this metric.
        if operation == 2 || operation == 3 {
            drop(state.runtime.take());
        }
        if operation == 3 {
            #[cfg(feature = "owned")]
            state.keys.push(
                ic_storage::StableKey::parse(format!("bench.aaa{}.v1", state.count)).unwrap(),
            );
            state.count += 1;
        }
        let start = ic0::performance_counter(0);
        let value = match operation {
            11 => 0,
            0 => state.memory(0).size(),
            1 => (0..state.count).map(|i| state.memory(i).size()).sum(),
            6 => state.held.as_ref().unwrap().size(),
            7 | 12 => {
                let memory = state.held.as_ref().unwrap();
                if memory.size() == 0 {
                    0
                } else {
                    let mut total = 0;
                    for _ in 0..if operation == 12 { 32 } else { 1 } {
                        let mut bytes = [0; 8];
                        memory.read(0, &mut bytes);
                        total += u64::from_le_bytes(bytes);
                    }
                    total
                }
            }
            8 => {
                assert!(state.held.is_none());
                state.held = Some(state.memory(0));
                state.held.as_ref().unwrap().size()
            }
            9 => {
                drop(state.held.take().unwrap());
                0
            }
            #[cfg(feature = "owned")]
            10 => {
                state
                    .runtime
                    .as_ref()
                    .unwrap()
                    .diagnostics()
                    .unwrap()
                    .data_pages
            }
            4 => {
                // Retain the complete group on both backends, including Drop
                // costs, without adding a second admission API.
                let memories: Vec<_> = (0..state.count).map(|i| state.memory(i)).collect();
                memories.iter().map(Memory::size).sum()
            }
            2 => {
                state.runtime = Some(runtime());
                0
            }
            3 => {
                state.runtime = Some(runtime());
                state.admit();
                state.memory(state.count - 1).size()
            }
            _ => panic!("unknown admission operation"),
        };
        let cost = ic0::performance_counter(0) - start;
        if operation == 2 {
            state.admit();
        }
        // Contents checked outside timing, including after every cold reopen.
        let opened;
        let memory = if let Some(memory) = &state.held {
            memory
        } else {
            opened = state.memory(0);
            &opened
        };
        let pages = memory.size();
        if pages != 0 {
            let mut bytes = [0; 8];
            memory.read(0, &mut bytes);
            assert_eq!(u64::from_le_bytes(bytes), 42);
        }
        for number in [cost, value, pages] {
            ic0::msg_reply_data_append(&number.to_le_bytes());
        }
        ic0::msg_reply();
    });
}
