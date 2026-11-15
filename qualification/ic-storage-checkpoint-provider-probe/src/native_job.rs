//! Controller-authorized, single-use native migration job fixture.
use crate::provider::job_cell;
use ic_platform_runtime::ic0;
use ic_storage_native_feasibility::migration_job::Job;

#[expect(unsafe_code, reason = "scalar imports bound to reviewed WAT callbacks")]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn domain() -> u64;
    safe fn freeze() -> u32;
    safe fn counter() -> u64;
    safe fn value(index: u32) -> u64;
    safe fn fresh();
    safe fn begin(counter: u64);
    safe fn put(index: u32, value: u64);
    safe fn finish(length: u32);
}

fn controller() {
    let len = ic0::msg_caller_size();
    assert!(len <= 29);
    let mut bytes = [0; 29];
    ic0::msg_caller_copy(&mut bytes[..len], 0);
    assert!(
        ic0::is_controller(&bytes[..len]) == 1,
        "migration controller required"
    );
}

fn args() -> [u64; 3] {
    assert_eq!(ic0::msg_arg_data_size(), 24);
    let mut bytes = [0; 24];
    ic0::msg_arg_data_copy(&mut bytes, 0);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}

#[expect(unsafe_code, reason = "private entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_create() {
    job_cell(true).set(Job::initial(domain()).unwrap().encode().unwrap());
}

#[expect(unsafe_code, reason = "private entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_export() {
    controller();
    let [epoch, target, fault] = args();
    assert!(fault <= 1);
    let mut cell = job_cell(false);
    let job = Job::decode(cell.get()).unwrap();
    let count = freeze();
    assert!(count <= 512);
    let values: Vec<_> = (0..count).map(|i| value(i)).collect();
    let next = job
        .export(domain(), target, epoch, counter(), &values)
        .unwrap();
    cell.set(next.encode().unwrap());
    if fault == 1 {
        ic0::trap(b"job export after stable write");
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private recovery entry bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_restore() {
    let [epoch, source, fault] = args();
    assert!(fault <= 3);
    fresh();
    let mut cell = job_cell(false);
    let mut job = Job::decode(cell.get()).unwrap();
    let decoded = job.consume(domain(), source, epoch).unwrap();
    if fault == 1 {
        cell.set(job.encode().unwrap());
        ic0::trap(b"job restore after consumed write");
    }
    begin(decoded.counter);
    for (i, value) in decoded.values.iter().enumerate() {
        put(i as u32, *value);
        if fault == 2 {
            ic0::trap(b"job restore during construction");
        }
    }
    finish(decoded.values.len() as u32);
    cell.set(job.encode().unwrap());
    if fault == 3 {
        ic0::trap(b"job restore after native publication");
    }
}

#[expect(unsafe_code, reason = "private evidence query bound by final composer")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_bytes() {
    ic0::msg_reply_data_append(job_cell(false).get());
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private governed fault injection entry")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_damage() {
    controller();
    assert_eq!(args(), [0, 0, 0]);
    freeze();
    let mut cell = job_cell(false);
    let mut bytes = cell.get().clone();
    bytes[24] ^= 1;
    cell.set(bytes);
    ic0::msg_reply();
}
