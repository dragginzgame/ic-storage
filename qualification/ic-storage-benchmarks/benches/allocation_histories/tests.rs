use super::History;

#[test]
fn mixed_histories_have_declared_growth_fractions_and_keep_small_rows_live() {
    for (history, grown) in [(History::Mixed10, 103), (History::Mixed50, 512)] {
        assert!((0..1024).all(|key| history.size(key, 0) == 33));
        assert_eq!(
            (0..1024).filter(|key| history.size(*key, 1) == 132).count(),
            grown
        );
        assert!((0..1024).all(|key| history.size(key, 2) == 33));
    }
    assert_eq!(
        (0..24)
            .filter(|round| History::GrowOnce.size(0, *round) == 132)
            .count(),
        1
    );
    assert_eq!(History::InlineReset.size(0, 1), 0);
    assert_eq!(History::Staircase.size(0, 7), 4096);
}
