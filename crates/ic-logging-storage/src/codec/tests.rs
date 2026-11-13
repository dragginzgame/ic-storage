//! Retained-record fixtures deliberately bypass canonical engine admission.
use super::*;

pub fn encode_snapshot(
    identity: LogIdentity,
    state: &ic_logging::LogSnapshot,
) -> Result<Vec<u8>, StoreError> {
    struct Rows<'a>(&'a [Entry]);
    impl Serialize for Rows<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serialize_entries(self.0, serializer)
        }
    }
    if state.entries.len() > MAX_ENTRIES {
        return Err(StoreError::Codec);
    }
    encode_checkpoint(&Checkpoint {
        epoch: identity.epoch,
        limits: state.limits,
        next_sequence: state.next_sequence,
        last_created_at_secs: state.last_created_at_secs,
        entries: Rows(&state.entries),
    })
}
