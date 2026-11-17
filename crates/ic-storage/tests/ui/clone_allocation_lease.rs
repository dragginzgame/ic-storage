use ic_storage::{RuntimeMemory, VectorMemory};

fn duplicate(memory: RuntimeMemory<VectorMemory>) {
    let _other = memory.clone();
}

fn main() {}
