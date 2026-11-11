use super::*;

#[test]
fn claim_churn_preserves_other_backings_and_refuses_aliases_without_io() {
    const COUNT: usize = 257;
    let memories: Vec<_> = (0..COUNT).map(|_| VectorMemory::default()).collect();
    let mut claims: Vec<_> = (0..COUNT).map(|_| None).collect();
    // Coprime strides exercise interleaved admission and release independently
    // of the allocator's address order. Identity comparisons stay opaque.
    for step in 0..COUNT {
        let index = step * 73 % COUNT;
        claims[index] = Some(BackingClaim::acquire(&memories[index]).unwrap());
        let alias = Rc::new(memories[index].clone());
        assert!(matches!(
            BackingClaim::acquire(&alias),
            Err(RuntimeConstructionError::BackingAlreadyClaimed)
        ));
    }
    for step in 0..COUNT {
        let index = step * 149 % COUNT;
        drop(claims[index].take().unwrap());
        let reclaimed = BackingClaim::acquire(&memories[index]).unwrap();
        for other in [0, COUNT / 2, COUNT - 1] {
            if claims[other].is_some() || other == index {
                assert!(matches!(
                    BackingClaim::acquire(&memories[other]),
                    Err(RuntimeConstructionError::BackingAlreadyClaimed)
                ));
            }
        }
        drop(reclaimed);
    }
    for memory in memories {
        assert!(memory.borrow().is_empty());
        drop(BackingClaim::acquire(&memory).unwrap());
    }
}
