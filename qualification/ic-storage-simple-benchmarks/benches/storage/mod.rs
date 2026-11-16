//! Published Storage scalar/sequence backend; all memory uses governed handles.

use std::{cell::RefCell, convert::Infallible};

use ic_platform_runtime::raw as ic0;
use ic_storage::{
    AllocationGrants, DefaultMemoryImpl, GenericAllocationPolicy, MemoryAuthority, MemoryRequest,
    MemoryRuntime, SealedDeclarationSnapshot, StableKey,
    adapter::{StableCell, StableVec},
};
use ic_storage_simple_benchmarks::{Access, run, wire};

struct State {
    cell: StableCell<u64>,
    sequence: StableVec<u64>,
}
thread_local! {static STATE: RefCell<Option<State>> = const {RefCell::new(None)};}

impl Access for State {
    type Error = Infallible;
    fn scalar(&self) -> Result<u64, Infallible> {
        Ok(*self.cell.get())
    }
    fn set_scalar(&mut self, value: u64) -> Result<(), Infallible> {
        self.cell.set(value);
        Ok(())
    }
    fn scan(&self) -> Result<u64, Infallible> {
        Ok((0..self.sequence.len())
            .map(|i| self.sequence.get(i).unwrap())
            .fold(0u64, u64::wrapping_add))
    }
    fn replace(&mut self, base: u64) -> Result<(), Infallible> {
        for i in 0..self.sequence.len() {
            self.sequence.set(i, &(base + i));
        }
        Ok(())
    }
    fn append(&mut self, value: u64) -> Result<(), Infallible> {
        self.sequence.push(&value);
        Ok(())
    }
}

#[expect(unsafe_code, reason = "unique benchmark install export")]
#[unsafe(export_name = "canister_init")]
extern "C" fn init() {
    let (rows, seed) = wire::setup();
    let mut runtime = MemoryRuntime::new(DefaultMemoryImpl::default()).unwrap();
    let declarations = SealedDeclarationSnapshot::new(vec![
        MemoryRequest::new("simple", "simple.cell.v1").unwrap(),
        MemoryRequest::new("simple", "simple.sequence.v1").unwrap(),
    ])
    .unwrap();
    let grants =
        AllocationGrants::new(vec![MemoryAuthority::new("simple", "simple.").unwrap()]).unwrap();
    runtime
        .bootstrap(&declarations, &grants, &GenericAllocationPolicy)
        .unwrap();
    let cell = StableCell::create(
        runtime
            .open_memory(&StableKey::parse("simple.cell.v1").unwrap())
            .unwrap(),
        seed,
    )
    .unwrap();
    let sequence = StableVec::create(
        runtime
            .open_memory(&StableKey::parse("simple.sequence.v1").unwrap())
            .unwrap(),
    )
    .unwrap();
    for i in 0..rows {
        sequence.push(&(seed * 100 + i));
    }
    STATE.with(|slot| *slot.borrow_mut() = Some(State { cell, sequence }));
}

#[expect(unsafe_code, reason = "unique benchmark update export")]
#[unsafe(export_name = "canister_update bench_run")]
extern "C" fn bench_run() {
    let (operation, batch) = wire::request();
    let start = ic0::instruction_counter();
    let result =
        STATE.with(|slot| run(slot.borrow_mut().as_mut().unwrap(), operation, batch).unwrap());
    let instructions = ic0::instruction_counter() - start;
    wire::reply(&[instructions, result]);
}

#[expect(unsafe_code, reason = "unique benchmark evidence query export")]
#[unsafe(export_name = "canister_query bench_state")]
extern "C" fn bench_state() {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot.as_ref().unwrap();
        let mut words = [0; 128];
        let length = state.sequence.len() as usize;
        assert!(length < words.len());
        words[0] = *state.cell.get();
        for i in 0..length {
            words[i + 1] = state.sequence.get(i as u64).unwrap();
        }
        wire::reply(&words[..length + 1]);
    });
}
