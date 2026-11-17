//! Independent native-slot observations for the sequential scalar reader.
use super::installed_tests::{query, word};
use ic_testkit::pocket_ic::{CanisterId, nonblocking::PocketIc};

/// Independent oracle from retained slots, not a map rebuilt by either codec.
pub(super) async fn check_prefix(pic: &PocketIc, id: CanisterId) {
    use ic_storage_retention_probe::artifact::sha256;
    let native = query(pic, id, "observe").await;
    let reply = query(pic, id, "native_prefix").await;
    assert_eq!(reply.len(), 80);
    for root_id in 0..2 {
        let root = word(&native, 56 + root_id * 8);
        let start = 104 + (root >> 16) as usize * 24;
        let buckets = 1usize << ((root >> 12) & 15);
        let slots: Vec<_> = native[start..start + buckets * 24]
            .as_chunks::<24>()
            .0
            .iter()
            .filter(|slot| word(*slot, 0) == 1)
            .collect();
        let mut portable = Vec::from(*b"NKEYMAP1");
        portable.extend_from_slice(&0u64.to_le_bytes());
        portable.extend_from_slice(&(slots.len() as u64).to_le_bytes());
        for slot in &slots {
            portable.extend_from_slice(&slot[8..24]);
        }
        portable.extend_from_slice(&sha256(&portable));
        let mut observed = Vec::from(*b"NKPNAT01");
        observed.extend_from_slice(&portable);
        assert_eq!(word(&reply, root_id * 40) as usize, slots.len());
        assert_eq!(
            &reply[root_id * 40 + 8..(root_id + 1) * 40],
            &sha256(&observed)
        );
    }
    assert_eq!(query(pic, id, "observe").await, native);
}

pub(super) async fn check(pic: &PocketIc, id: CanisterId) {
    let native = query(pic, id, "observe").await;
    let reply = query(pic, id, "scan_records").await;
    let mut offset = 0;
    for root_id in 0..2 {
        let root = word(&native, 56 + root_id * 8);
        let start = 104 + (root >> 16) as usize * 24;
        let buckets = 1usize << ((root >> 12) & 15);
        let slots = native[start..start + buckets * 24].as_chunks::<24>().0;
        let occupied: Vec<_> = slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| word(*slot, 0) == 1)
            .collect();
        assert_eq!(word(&reply, offset) as usize, occupied.len());
        offset += 8;
        for (_, slot) in &occupied {
            assert_eq!(&reply[offset..offset + 16], &slot[8..24]);
            offset += 16;
        }
        let visits = word(&reply, offset) as usize;
        offset += 8;
        let expected_visits = occupied.last().map_or(0, |(slot, _)| slot + 1);
        assert_eq!(visits, expected_visits);
        assert!(visits <= buckets);
        let prior_visits: usize = occupied.iter().map(|(slot, _)| 2 * (slot + 1)).sum();
        println!(
            "map scalar scan root={root_id} records={} buckets={buckets} visits={visits} prior_rescan_visits={prior_visits}",
            occupied.len()
        );
        if !occupied.is_empty() {
            assert!(visits < prior_visits);
        }
    }
    assert_eq!(offset, reply.len());
    assert_eq!(query(pic, id, "observe").await, native);
}
