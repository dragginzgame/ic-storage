//! Scalar record conversion in the transient checkpoint runtime.
//! WAT owns retained fields; no saved Rust record reference enters this module.
use super::{args, checkpoint, ic0};
use ic_storage_native_feasibility::record::{
    Error, Record, V1, V2,
    narrow::{self, V3},
};

#[expect(
    unsafe_code,
    reason = "scalar record callbacks bound by the reviewed composer"
)]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn record_schema() -> u32;
    safe fn record_ceiling() -> u64;
    safe fn record_restore(schema: u32, ceiling: u64);
    safe fn record_source(field: u32) -> u64;
    safe fn record_publish(ceiling: u64);
    safe fn record_read(field: u32) -> u64;
    safe fn record_write(counter: u64);
}

#[expect(
    unsafe_code,
    reason = "private record update entry bound by the composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_record_add() {
    let [delta, fault] = args();
    assert!(fault <= 1);
    // These callbacks first admit a ready, settled record. Rust works only
    // with scalar copies; the provider owns native publication and IC rollback.
    let mut candidate = match record_schema() {
        2 => Record::V2(V2::new(record_read(0), record_read(1)).unwrap()),
        3 => Record::V3(V3::new(u32::try_from(record_read(0)).unwrap(), record_read(1)).unwrap()),
        _ => ic0::trap(b"unsupported mutable record schema"),
    };
    let status: u64 = match &mut candidate {
        Record::V2(value) => match value.store_with(|mut view| view.add(delta)) {
            Ok(()) => 0,
            Err(Error::Ceiling) => 1,
            Err(Error::Overflow) => 2,
        },
        Record::V3(value) => match u32::try_from(delta)
            .map_err(|_| narrow::Error::Range)
            .and_then(|delta| value.store_with(|mut view| view.add(delta)))
        {
            Ok(()) => 0,
            Err(narrow::Error::Ceiling) => 1,
            Err(narrow::Error::Overflow) => 2,
            Err(narrow::Error::Range) => 3,
        },
        Record::V1(_) => ic0::trap(b"unsupported mutable record schema"),
    };
    if status == 0 {
        record_write(candidate.counter());
        if fault == 1 {
            ic0::trap(b"record update after publication");
        }
    }
    ic0::msg_reply_data_append(&status.to_le_bytes());
    ic0::msg_reply();
}

pub(super) fn encode(counter: u64, values: &[u64], epoch: u64) -> Vec<u8> {
    let record = match record_schema() {
        1 => Record::V1(V1 { counter }),
        2 => Record::V2(V2::new(counter, record_ceiling()).unwrap()),
        3 => Record::V3(V3::new(u32::try_from(counter).unwrap(), record_ceiling()).unwrap()),
        _ => ic0::trap(b"unsupported record schema"),
    };
    checkpoint::encode_record(record, values, epoch).unwrap()
}

pub(super) fn restore(record: Record) {
    let (schema, ceiling) = match record {
        Record::V1(_) => (1, 0),
        Record::V2(value) => (2, value.ceiling()),
        Record::V3(value) => (3, value.ceiling()),
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
        counter: record_source(0),
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

#[expect(
    unsafe_code,
    reason = "private checked narrowing entry bound only to an upgrade"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_narrow() {
    let [reserved, fault] = args();
    assert_eq!(reserved, 0);
    assert!(fault <= 2);
    let source = V2::new(record_source(0), record_source(1)).unwrap();
    let target =
        V3::try_from(source).unwrap_or_else(|_| ic0::trap(b"record narrowing out of range"));
    if fault == 1 {
        ic0::trap(b"record narrowing before publication");
    }
    record_publish(target.ceiling());
    if fault == 2 {
        ic0::trap(b"record narrowing after publication");
    }
}

#[expect(
    unsafe_code,
    reason = "private exact widening entry bound only to an upgrade"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_widen() {
    let [reserved, fault] = args();
    assert_eq!(reserved, 0);
    assert!(fault <= 2);
    let source = V3::new(u32::try_from(record_source(0)).unwrap(), record_source(1)).unwrap();
    let target = V2::from(source);
    if fault == 1 {
        ic0::trap(b"record widening before publication");
    }
    record_publish(target.ceiling());
    if fault == 2 {
        ic0::trap(b"record widening after publication");
    }
}
