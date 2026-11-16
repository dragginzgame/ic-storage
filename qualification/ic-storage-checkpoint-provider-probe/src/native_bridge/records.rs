//! Scalar record conversion in the transient checkpoint runtime.
//! WAT owns retained fields; no saved Rust record reference enters this module.
use super::{args, checkpoint, ic0};
use ic_storage_native_feasibility::record::{Record, V1, V2};

#[expect(
    unsafe_code,
    reason = "scalar record callbacks bound by the reviewed composer"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn record_schema() -> u32;
    safe fn record_ceiling() -> u64;
    safe fn record_restore(schema: u32, ceiling: u64);
    safe fn record_source_counter() -> u64;
    safe fn record_publish(ceiling: u64);
}

pub(super) fn encode(counter: u64, values: &[u64], epoch: u64) -> Vec<u8> {
    let record = match record_schema() {
        1 => Record::V1(V1 { counter }),
        2 => Record::V2(V2::new(counter, record_ceiling()).unwrap()),
        _ => ic0::trap(b"unsupported record schema"),
    };
    checkpoint::encode_record(record, values, epoch).unwrap()
}

pub(super) fn restore(record: Record) {
    let (schema, ceiling) = match record {
        Record::V1(_) => (1, 0),
        Record::V2(value) => (2, value.ceiling()),
    };
    record_restore(schema, ceiling);
}

#[expect(
    unsafe_code,
    reason = "private record conversion entry bound only to an upgrade"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_convert() {
    let [ceiling, fault] = args();
    assert!(fault <= 2);
    let source = V1 {
        counter: record_source_counter(),
    };
    let target = source.upgrade(ceiling).unwrap();
    if fault == 1 {
        ic0::trap(b"record conversion before publication");
    }
    record_publish(target.ceiling());
    if fault == 2 {
        ic0::trap(b"record conversion after publication");
    }
}
