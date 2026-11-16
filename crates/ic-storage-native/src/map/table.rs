//! Native open addressing, with a maximum 75% load and backward-shift deletion.

use super::{Error, Slot};

pub(super) fn capacity(buckets: usize) -> usize {
    buckets - buckets / 4
}

pub(super) fn buckets_for(entries: usize) -> Result<usize, Error> {
    let mut buckets = 8usize;
    while capacity(buckets) < entries {
        buckets = buckets.checked_mul(2).ok_or(Error::Allocation)?;
    }
    Ok(buckets)
}

// Fixed avalanche mixer: keys, never process addresses or randomized host state,
// determine the retained probe placement. This is not an adversarial-key defense.
pub(super) fn home(key: u64, mask: usize) -> usize {
    let mut value = key;
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    value as usize & mask
}

pub(super) fn find(slots: &[Slot], key: u64) -> Option<usize> {
    if slots.is_empty() {
        return None;
    }
    let mask = slots.len() - 1;
    let mut index = home(key, mask);
    for _ in 0..slots.len() {
        let slot = slots[index];
        if slot.occupied == 0 {
            return None;
        }
        if slot.key == key {
            return Some(index);
        }
        index = (index + 1) & mask;
    }
    None
}

pub(super) fn place(slots: &mut [Slot], value: Slot) {
    let mask = slots.len() - 1;
    let mut index = home(value.key, mask);
    // All callers reserve below the load bound before inserting a new key.
    while slots[index].occupied != 0 {
        index = (index + 1) & mask;
    }
    slots[index] = value;
}

pub(super) fn remove(slots: &mut [Slot], mut hole: usize) {
    let mask = slots.len() - 1;
    let mut index = (hole + 1) & mask;
    while slots[index].occupied != 0 {
        let start = home(slots[index].key, mask);
        // Move only entries whose probe path crosses the hole, including wrap.
        if index.wrapping_sub(start) & mask >= hole.wrapping_sub(start) & mask {
            slots[hole] = slots[index];
            hole = index;
        }
        index = (index + 1) & mask;
    }
    slots[hole] = Slot::EMPTY;
}

pub(super) fn validate(slots: &[Slot], len: usize, limit: usize) -> Result<(), Error> {
    if len > limit {
        return Err(Error::Limit);
    }
    if slots.is_empty() {
        return if len == 0 {
            Ok(())
        } else {
            Err(Error::NativeImage)
        };
    }
    if slots.len() < 8 || !slots.len().is_power_of_two() || len > capacity(slots.len()) {
        return Err(Error::NativeImage);
    }
    let mut count = 0;
    for (index, slot) in slots.iter().enumerate() {
        match slot.occupied {
            0 if slot.key == 0 && slot.value == 0 => {}
            1 if find(slots, slot.key) == Some(index) => count += 1,
            _ => return Err(Error::NativeImage),
        }
    }
    if count == len {
        Ok(())
    } else {
        Err(Error::NativeImage)
    }
}
