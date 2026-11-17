//! Bounded fixture recovery envelope binding all native roots and declarations.
use super::ordered_owner::ROOTS;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

const HEADER: usize = 16 + 4 * 56;
const MAX: usize = 1024 * 1024;
fn declaration(index: usize, length: usize) -> [u64; 7] {
    let root = ROOTS[index];
    [
        root.id,
        root.participant,
        root.schema,
        root.entries as u64,
        root.key_bytes as u64,
        root.value_bytes as u64,
        length as u64,
    ]
}
pub(super) fn encode(epoch: u64, parts: &[Vec<u8>; 4]) -> Vec<u8> {
    let size = HEADER + 32 + parts.iter().map(Vec::len).sum::<usize>();
    assert!(size <= MAX);
    let mut result = Vec::with_capacity(size);
    result.extend_from_slice(b"ORDFAM04");
    result.extend_from_slice(&epoch.to_le_bytes());
    for (index, part) in parts.iter().enumerate() {
        for word in declaration(index, part.len()) {
            result.extend_from_slice(&word.to_le_bytes());
        }
    }
    for part in parts {
        result.extend_from_slice(part);
    }
    let seal = Sha256::digest(&result);
    result.extend_from_slice(&seal);
    result
}
pub(super) fn decode(bytes: &[u8], epoch: u64) -> [&[u8]; 4] {
    assert!((HEADER + 32..=MAX).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ORDFAM04");
    assert_eq!(&bytes[8..16], &epoch.to_le_bytes());
    let end = bytes.len() - 32;
    assert_eq!(Sha256::digest(&bytes[..end]).as_slice(), &bytes[end..]);
    let mut at = HEADER;
    core::array::from_fn(|index| {
        let start = 16 + index * 56;
        let words = bytes[start..start + 56].as_chunks::<8>().0;
        let length = usize::try_from(u64::from_le_bytes(words[6])).unwrap();
        assert_eq!(
            core::array::from_fn::<_, 7, _>(|i| u64::from_le_bytes(words[i])),
            declaration(index, length)
        );
        let next = at.checked_add(length).filter(|next| *next <= end).unwrap();
        let part = &bytes[at..next];
        at = next;
        if index == 3 {
            assert_eq!(at, end);
        }
        part
    })
}

// Keep the outer seal valid so installed refusal reaches declaration validation
// or the final nested map after earlier recovery candidates have been written.
pub(super) fn corrupt_checkpoint(bytes: &mut [u8], kind: u64) {
    let end = bytes.len() - 32;
    if kind == 7 {
        bytes[16 + 3 * 56 + 40] ^= 1; // jobs' declared value bound
    } else {
        assert_eq!(kind, 8);
        bytes[end - 1] ^= 1; // final nested map's own seal
    }
    let seal = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&seal);
}

fn key(slot: u64) -> [u8; 512] {
    assert!(slot < 32);
    let mut key = [0xa5; 512];
    key[..8].copy_from_slice(&slot.to_be_bytes());
    key
}

#[expect(unsafe_code, reason = "closed complete-family logical evidence")]
#[unsafe(export_name = "canister_query family_records")]
extern "C" fn records() {
    super::ordered_runtime::enter();
    super::ordered_jobs::available();
    super::ordered_session::with(|owner| {
        ic_platform_runtime::raw::msg_reply_data_append(&owner.export(0));
    });
    ic_platform_runtime::raw::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "closed composed four-root mutation and trap injection"
)]
#[unsafe(export_name = "canister_update family_write")]
extern "C" fn write() {
    super::ordered_runtime::enter();
    let start = ic_platform_runtime::raw::instruction_counter();
    let [slot, value, fault] = super::args();
    assert!(value <= 255 && fault <= 6);
    assert_eq!(super::checked()[9], 0, "frozen");
    let key = key(slot);
    super::ordered_session::with(|owner| {
        // Main's independent grant cannot read or mutate the other roots.
        owner
            .read(|view| assert!(view.contains_key(2, &key).is_err()))
            .unwrap();
        owner
            .update(|mut view| assert!(view.insert(3, &key, &[]).is_err()))
            .unwrap();
        owner
            .compose(|mut view| {
                view.insert(1, &key, &[value as u8; 4096]).unwrap();
                assert_ne!(fault, 1);
                if !view.read().contains_key(2, &key).unwrap() {
                    view.insert(2, &key, &[slot as u8; 4096]).unwrap();
                }
                assert_ne!(fault, 2);
                view.insert(3, &key, &[value as u8; 4096]).unwrap();
                assert_ne!(fault, 3);
                view.insert(4, &[slot as u8], &[value as u8; 16384])
                    .unwrap();
                assert_ne!(fault, 4);
                if fault == 6 {
                    // The shared carrier can hold this, but main's declaration cannot.
                    view.insert(1, &key, &[0; 4097]).unwrap();
                }
            })
            .unwrap();
    });
    let reply = super::encode_reply(value, fault == 5)
        .unwrap_or_else(|message| ic_platform_runtime::raw::trap(message.as_bytes()));
    ic_platform_runtime::raw::msg_reply_data_append(&reply);
    super::finish_cost(start);
    ic_platform_runtime::raw::msg_reply();
}

#[expect(unsafe_code, reason = "query isolation of all four roots")]
#[unsafe(export_name = "canister_query query_family_write")]
extern "C" fn query_write() {
    write();
}

#[expect(unsafe_code, reason = "bounded cleanup of one target and job pair")]
#[unsafe(export_name = "canister_update family_cleanup")]
extern "C" fn cleanup() {
    super::ordered_runtime::enter();
    let [slot, _, fault] = super::args();
    assert!(fault <= 1);
    assert_eq!(super::checked()[9], 0, "frozen");
    let key = key(slot);
    super::ordered_session::with(|owner| {
        owner
            .compose(|mut view| {
                view.remove_discard(3, &key).unwrap();
                assert_eq!(fault, 0);
                view.remove_discard(4, &[slot as u8]).unwrap();
            })
            .unwrap()
    });
    ic_platform_runtime::raw::msg_reply();
}
