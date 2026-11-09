//! Deterministic allocation histories; no native timing claims.
use super::{Backing, Owned, Stores, checkpoint};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
enum History {
    Fixed33,
    Fixed80,
    Toggle33,
    Toggle80,
    GrowOnce,
    Mixed10,
    Mixed50,
    Staircase,
    InlineReset,
}
impl History {
    fn size(self, key: u64, round: u32) -> usize {
        match self {
            Self::Fixed33 => 33,
            Self::Fixed80 => 80,
            Self::Toggle33 => {
                if round.is_multiple_of(2) {
                    33
                } else {
                    132
                }
            }
            Self::Toggle80 => {
                if round.is_multiple_of(2) {
                    80
                } else {
                    320
                }
            }
            Self::GrowOnce => {
                if round == 1 {
                    132
                } else {
                    33
                }
            }
            Self::Mixed10 => {
                if key.is_multiple_of(10) && round % 2 == 1 {
                    132
                } else {
                    33
                }
            }
            Self::Mixed50 => {
                if key.is_multiple_of(2) && round % 2 == 1 {
                    132
                } else {
                    33
                }
            }
            Self::Staircase => [33, 44, 45, 80, 108, 109, 132, 4096][round as usize % 8],
            Self::InlineReset => {
                if round.is_multiple_of(2) {
                    33
                } else {
                    0
                }
            }
        }
    }
}

pub(super) fn histories() -> Value {
    let mut runs = Vec::new();
    for history in [
        History::Fixed33,
        History::Fixed80,
        History::Toggle33,
        History::Toggle80,
        History::GrowOnce,
        History::Mixed10,
        History::Mixed50,
        History::Staircase,
        History::InlineReset,
    ] {
        let backing = Backing::default();
        let mut stores = Owned::new(backing.clone(), 4);
        stores.init();
        let mut model = BTreeMap::new();
        let mut phases = Vec::new();
        backing.0.reset();
        for round in 0..24_u32 {
            for n in 0..1024_u64 {
                // Odd multiplier permutes this power-of-two population.
                let key = (n * 4051) % 1024;
                for store in 0..4 {
                    let bytes = vec![u8::try_from(round).unwrap(); history.size(key, round)];
                    assert_eq!(
                        stores.put(store, key, bytes.clone()),
                        model.insert((store, key), bytes)
                    );
                }
            }
            let live = model.values().map(Vec::len).sum();
            phases.push(checkpoint(
                &stores,
                &backing,
                &format!("round-{round}"),
                live,
            ));
            // Cold reopen and independent full contents every fourth phase.
            if round % 4 == 3 {
                stores.maps.clear();
                for store in 0..4 {
                    let key =
                        ic_storage::StableKey::parse(format!("experiment.store{store:03}.v1"))
                            .unwrap();
                    stores.maps.push(
                        ic_storage::adapter::StableBTreeMap::load(
                            stores.runtime.open_memory(&key).unwrap(),
                        )
                        .unwrap(),
                    );
                }
                for (&(store, key), value) in &model {
                    assert_eq!(stores.get(store, key).as_ref(), Some(value));
                }
                backing.0.reset();
            }
        }
        for (&(store, key), value) in &model {
            assert_eq!(stores.remove(store, key).as_ref(), Some(value));
        }
        phases.push(checkpoint(&stores, &backing, "empty", 0));
        for &(store, key) in model.keys() {
            assert!(stores.put(store, key, vec![9; 33]).is_none());
        }
        phases.push(checkpoint(&stores, &backing, "refill-small", 4096 * 33));
        for &(store, key) in model.keys() {
            assert_eq!(stores.get(store, key), Some(vec![9; 33]));
        }
        runs.push(json!({"history":format!("{history:?}"),"phases":phases}));
    }
    json!({"scope":"Native capacity and IO, not IC instructions. Four governed maps with 1024 records each; 24 deterministic size phases, six cold reopen/full-state checks, complete deletion then small refill. Diagnostic/verification IO excluded from following phase.","runs":runs})
}

#[cfg(test)]
mod tests;
