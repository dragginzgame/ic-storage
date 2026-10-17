//! Shared runner/canister protocol. Discriminants are the existing u8 wire IDs.

pub const SEED_MAX_BATCH: u64 = 32;
#[cfg(not(target_arch = "wasm32"))]
pub const SEED_MAX_CALLS: u64 = 2048;
#[cfg(not(target_arch = "wasm32"))]
pub const SEED_BYTE_BUDGET: usize = 524_288;
#[cfg(any(target_arch = "wasm32", test))]
pub const SCAN_LIMIT: usize = 16;
pub const WORKLOAD_RECORDS: u64 = 512;
pub const WORKLOAD_SEED_BATCH: u64 = 16;
#[cfg(not(target_arch = "wasm32"))]
pub const WORKLOAD_SEED_CALLS: u64 = WORKLOAD_RECORDS.div_ceil(WORKLOAD_SEED_BATCH);
#[cfg(any(target_arch = "wasm32", test))]
pub const WORKLOAD_BATCH: u64 = 32;
pub const WORKLOAD_ROUNDS: u8 = 4;
pub const WORKLOAD_PROFILES: [&str; 2] = ["mixed_rows", "mixed_index"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MatrixOperation {
    Get = 0,
    Replace = 1,
    ScanKeys = 2,
    Remove = 3,
    MissingGet = 4,
    ScanValues = 5,
    Shrink = 6,
    Regrow = 7,
    Refill = 8,
    InsertNew = 9,
    ReplaceChanged = 10,
}

pub const MATRIX_OPERATIONS: [(MatrixOperation, &str); 11] = {
    use MatrixOperation::{
        Get, InsertNew, MissingGet, Refill, Regrow, Remove, Replace, ReplaceChanged, ScanKeys,
        ScanValues, Shrink,
    };
    [
        (Get, "get"),
        (MissingGet, "missing_get"),
        (Replace, "replace"),
        (ReplaceChanged, "replace_changed"),
        (ScanKeys, "scan_keys"),
        (ScanValues, "scan_values"),
        (Shrink, "shrink"),
        (Regrow, "regrow"),
        (Remove, "remove"),
        (Refill, "refill"),
        (InsertNew, "insert_new"),
    ]
};
impl TryFrom<u8> for MatrixOperation {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        MATRIX_OPERATIONS
            .iter()
            .find_map(|&(op, _)| (op as u8 == value).then_some(op))
            .ok_or("invalid matrix operation")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WorkloadOperation {
    ReadMix = 0,
    FoldPut = 1,
    FoldDelete = 2,
    Refill = 3,
    KeysForward = 4,
    KeysReverse = 5,
    ValuesForward = 6,
    ValuesReverse = 7,
}

pub const WORKLOAD_OPERATIONS: [(WorkloadOperation, &str); 8] = {
    use WorkloadOperation::{
        FoldDelete, FoldPut, KeysForward, KeysReverse, ReadMix, Refill, ValuesForward,
        ValuesReverse,
    };
    [
        (ReadMix, "read_mix"),
        (FoldPut, "fold_put"),
        (FoldDelete, "fold_delete"),
        (Refill, "refill"),
        (KeysForward, "keys_forward"),
        (KeysReverse, "keys_reverse"),
        (ValuesForward, "values_forward"),
        (ValuesReverse, "values_reverse"),
    ]
};
impl TryFrom<u8> for WorkloadOperation {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        WORKLOAD_OPERATIONS
            .iter()
            .find_map(|&(op, _)| (op as u8 == value).then_some(op))
            .ok_or("invalid workload operation")
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn wire_ids_labels_and_seed_schedule_match_existing_protocol() {
        assert_eq!(
            MATRIX_OPERATIONS.map(|(op, label)| (op as u8, label)),
            [
                (0, "get"),
                (4, "missing_get"),
                (1, "replace"),
                (10, "replace_changed"),
                (2, "scan_keys"),
                (5, "scan_values"),
                (6, "shrink"),
                (7, "regrow"),
                (3, "remove"),
                (8, "refill"),
                (9, "insert_new"),
            ]
        );
        assert_eq!(
            WORKLOAD_OPERATIONS.map(|(op, label)| (op as u8, label)),
            [
                (0, "read_mix"),
                (1, "fold_put"),
                (2, "fold_delete"),
                (3, "refill"),
                (4, "keys_forward"),
                (5, "keys_reverse"),
                (6, "values_forward"),
                (7, "values_reverse"),
            ]
        );
        for (op, _) in MATRIX_OPERATIONS {
            assert_eq!(MatrixOperation::try_from(op as u8), Ok(op));
        }
        for (op, _) in WORKLOAD_OPERATIONS {
            assert_eq!(WorkloadOperation::try_from(op as u8), Ok(op));
        }
        assert!(MatrixOperation::try_from(11).is_err());
        assert!(MatrixOperation::try_from(255).is_err());
        assert!(WorkloadOperation::try_from(8).is_err());
        assert!(WorkloadOperation::try_from(255).is_err());
        assert_eq!(
            (WORKLOAD_RECORDS, WORKLOAD_SEED_BATCH, WORKLOAD_SEED_CALLS),
            (512, 16, 32)
        );
        assert_eq!((WORKLOAD_BATCH, WORKLOAD_ROUNDS, SCAN_LIMIT), (32, 4, 16));
        assert_eq!(WORKLOAD_PROFILES, ["mixed_rows", "mixed_index"]);
    }
}
