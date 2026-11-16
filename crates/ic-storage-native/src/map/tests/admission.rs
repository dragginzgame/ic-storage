use super::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
fn native_slot_contract_and_readmission_preserve_allocation_and_values() {
    assert_eq!(size_of::<Slot>(), 24);
    assert_eq!(align_of::<Slot>(), align_of::<u64>());
    assert_eq!(
        (
            offset_of!(Slot, occupied),
            offset_of!(Slot, key),
            offset_of!(Slot, value)
        ),
        (0, 8, 16)
    );
    let mut map = Map::new(100);
    map.with(|mut view| {
        for key in 0..100 {
            view.insert(key, key * 7).unwrap();
        }
    })
    .unwrap();
    let (slots, len) = map.release().unwrap();
    let pointer = slots.as_ptr();
    let mut reopened = Map::admit(slots, len, 100).unwrap_or_else(|_| panic!("valid admission"));
    assert_eq!(
        entries(&mut reopened),
        (0..100).map(|key| (key, key * 7)).collect()
    );
    assert_eq!(reopened.release().unwrap().0.as_ptr(), pointer);
}

#[test]
fn unknown_native_state_refuses_without_resetting_payload() {
    for fault in 0..7 {
        let mut map = Map::new(10);
        map.with(|mut view| {
            view.insert(0, 7).unwrap();
            view.insert(1, 8).unwrap();
        })
        .unwrap();
        let (mut slots, mut len) = map.release().unwrap();
        let mut limit = 10;
        let first = table::find(&slots, 0).unwrap();
        let empty = slots.iter().position(|slot| slot.occupied == 0).unwrap();
        match fault {
            0 => len += 1,
            1 => slots[first].occupied = 2,
            2 => slots[empty].key = 99,
            3 => {
                slots[empty] = slots[first];
                len += 1;
            }
            4 => {
                slots[empty] = slots[first];
                slots[first] = Slot::EMPTY;
            }
            5 => {
                slots.pop();
            }
            _ => limit = 1,
        }
        let before: Vec<_> = slots
            .iter()
            .map(|slot| (slot.occupied, slot.key, slot.value))
            .collect();
        let pointer = slots.as_ptr();
        let error = match Map::admit(slots, len, limit) {
            Ok(_) => panic!("bad image admitted"),
            Err(error) => error,
        };
        assert_eq!(
            error.error,
            if fault == 6 {
                Error::Limit
            } else {
                Error::NativeImage
            }
        );
        assert_eq!(error.slots.as_ptr(), pointer);
        assert_eq!(
            error
                .slots
                .iter()
                .map(|slot| (slot.occupied, slot.key, slot.value))
                .collect::<Vec<_>>(),
            before
        );
    }
    assert!(Map::admit(Vec::new(), 0, 0).is_ok());
    assert!(matches!(
        Map::admit(Vec::new(), 1, 1),
        Err(super::super::AdmissionError {
            error: Error::NativeImage,
            ..
        })
    ));
}
