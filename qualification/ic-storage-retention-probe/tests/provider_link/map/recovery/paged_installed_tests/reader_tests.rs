//! Independent checks of the scalar callback's custody and sequential protocol.
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn staged_reader_refuses_wrong_custody_and_out_of_order_scalars() {
    let (dir, a) = artifacts();
    let pic = server().await;
    let id = install(&pic, &a.source).await;
    let before = query(&pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    for mode in 0..=21 {
        let result = pic
            .query_call(id, id, &format!("reader_probe_{mode}"), vec![])
            .await;
        if mode < 2 {
            assert_eq!(result.unwrap(), Vec::<u8>::new());
        } else {
            assert_eq!(
                result.unwrap_err().error_code,
                ic_testkit::pocket_ic::ErrorCode::CanisterTrapped,
                "mode {mode}"
            );
        }
        assert_eq!(query(&pic, id, "observe").await, before);
        assert_eq!(pic.get_stable_memory(id).await, stable);
    }
    fs::write(
        dir.join("PAGED_NATIVE_READER"),
        b"staged scalar custody and ordering passed\n",
    )
    .unwrap();
    pic.drop().await;
}
