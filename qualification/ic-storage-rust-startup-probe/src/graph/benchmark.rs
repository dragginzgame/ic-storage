//! M0 sequence uses existing Node links, not a speculative vector implementation.

use ic_platform_runtime::raw as ic0;
use ic_storage_new::m0::image::{Counter, Error, Session};
use ic_storage_simple_benchmarks::{Access, run, wire};

use super::kernel;

pub(super) fn initialize() {
    let (rows, seed) = wire::setup();
    kernel::initialize();
    if !kernel::grow() {
        std::arch::wasm32::unreachable();
    }
    kernel::update(|s| {
        let mut head = None;
        for i in (0..rows).rev() {
            head = Some(s.allocate_node(seed * 100 + i, head)?);
        }
        let root = s.allocate_counter(seed, head, None)?;
        s.publish_root(root)
    });
}

struct Heap<'s, 'id> {
    session: &'s mut Session<'id>,
    root: Counter<'id>,
}

impl Access for Heap<'_, '_> {
    type Error = Error;
    fn scalar(&self) -> Result<u64, Error> {
        self.session.counter_value(self.root)
    }
    fn set_scalar(&mut self, value: u64) -> Result<(), Error> {
        self.session.set_counter_value(self.root, value)
    }
    fn scan(&self) -> Result<u64, Error> {
        let mut sum = 0u64;
        let mut next = self.session.left(self.root)?;
        while let Some(node) = next {
            sum = sum.wrapping_add(self.session.node_value(node)?);
            next = self.session.node_next(node)?;
        }
        Ok(sum)
    }
    fn replace(&mut self, base: u64) -> Result<(), Error> {
        let mut index = 0;
        let mut next = self.session.left(self.root)?;
        while let Some(node) = next {
            self.session.set_node_value(node, base + index)?;
            index += 1;
            next = self.session.node_next(node)?;
        }
        Ok(())
    }
    fn append(&mut self, value: u64) -> Result<(), Error> {
        let mut tail = self.session.left(self.root)?.ok_or(Error::Handle)?;
        while let Some(next) = self.session.node_next(tail)? {
            tail = next;
        }
        let node = self.session.allocate_node(value, None)?;
        self.session.set_node_next(tail, Some(node))
    }
}

#[expect(unsafe_code, reason = "unique benchmark update export")]
#[unsafe(export_name = "canister_update bench_run")]
extern "C" fn bench_run() {
    let (operation, batch) = wire::request();
    let start = ic0::instruction_counter();
    let result = kernel::update(|s| {
        let root = s.root()?.ok_or(Error::Handle)?;
        run(&mut Heap { session: s, root }, operation, batch)
    });
    let instructions = ic0::instruction_counter() - start;
    wire::reply(&[instructions, result]);
}

#[expect(unsafe_code, reason = "unique benchmark evidence query export")]
#[unsafe(export_name = "canister_query bench_state")]
extern "C" fn bench_state() {
    // Full ordered contents, read after timing and independently checked by host.
    let (words, length) = kernel::update(|s| {
        let root = s.root()?.ok_or(Error::Handle)?;
        let mut words = [0; 128];
        words[0] = s.counter_value(root)?;
        let mut length = 1;
        let mut next = s.left(root)?;
        while let Some(node) = next {
            if length == words.len() {
                return Err(Error::Capacity);
            }
            words[length] = s.node_value(node)?;
            length += 1;
            next = s.node_next(node)?;
        }
        Ok((words, length))
    });
    wire::reply(&words[..length]);
}
