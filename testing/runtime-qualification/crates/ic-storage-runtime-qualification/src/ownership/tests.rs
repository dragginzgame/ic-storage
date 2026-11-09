use super::*;
#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an explicitly prepared server URL and Memory fixture Wasm"]
async fn installed_backing_custody() {
    let pic = crate::connect_to_server().await;
    qualify(&pic).await;
    pic.drop().await.unwrap();
}
