use super::*;
use alloc::vec;

#[test]
fn scalar_operations_preserve_the_callers_owner_and_allocation() {
    let mut values = Vec::with_capacity(8);
    values.extend([17, 23, 41]);
    let pointer = values.as_ptr();
    let result = with_vector(&mut values, 8, |mut view| {
        view.values_mut()[0] += 1;
        assert_eq!(view.remove_first(), Some(18));
        view.push(99).unwrap();
        view.values().to_vec()
    })
    .unwrap();
    assert_eq!(result, [23, 41, 99]);
    assert_eq!(values.as_ptr(), pointer);
    assert_eq!(values, result);
    with_vector(&mut values, 8, |mut view| {
        view.shrink_to_fit();
        view.clear();
        assert_eq!(view.remove_first(), None);
        view.shrink_to_fit();
        view.push(7).unwrap();
    })
    .unwrap();
    assert_eq!(values, [7]);
}

#[test]
fn ordinary_errors_preserve_preceding_writes_and_limit_refusal_is_local() {
    let mut values = vec![17];
    let result = with_vector(&mut values, 2, |mut view| {
        view.values_mut()[0] = 23;
        view.push(41)?;
        view.push(99)
    });
    assert_eq!(result, Ok(Err(Refusal::Limit)));
    assert_eq!(values, [23, 41]);
    assert_eq!(
        with_vector(&mut values, 1, |_| panic!("oversized callback ran")),
        Err(Refusal::Limit)
    );
    assert_eq!(values, [23, 41]);
    let mut empty = Vec::new();
    assert_eq!(
        with_vector(&mut empty, 0, |mut view| view.push(1)),
        Ok(Err(Refusal::Limit))
    );
    assert_eq!(empty.capacity(), 0);
}

#[test]
fn forgotten_view_leaves_no_owner_or_finalization_obligation() {
    let mut values = vec![17];
    let pointer = values.as_ptr();
    with_vector(&mut values, 1, |mut view| {
        view.values_mut()[0] = 41;
        // This intentionally tests safe forgetting; the view owns only a borrow.
        #[expect(
            clippy::forget_non_drop,
            reason = "prove forgetting the scoped view is harmless"
        )]
        core::mem::forget(view);
    })
    .unwrap();
    assert_eq!(values.as_ptr(), pointer);
    with_vector(&mut values, 1, |mut view| view.values_mut()[0] += 1).unwrap();
    assert_eq!(values, [42]);
}
