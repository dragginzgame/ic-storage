#[tokio::test(flavor = "current_thread")]
#[ignore = "requires an explicitly prepared server URL and Storage fixture Wasm"]
async fn installed_storage_adapters() {
    let pic = crate::connect_to_server().await;
    super::qualify(&pic).await;
    pic.drop().await;
}
