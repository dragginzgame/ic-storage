//! Exercise access helpers as WAT only, before linking any Rust vector body.

use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};

use ic_testkit::pocket_ic::PocketIcBuilder;

use super::super::{admit, record, update};
use super::{install, observe, rejected_update, source};

fn candidate(root: (u64, u64), operation: u32, claim: bool, body: &str) -> Vec<u8> {
    let provider = source(1, root, 4096, false, "nop", true);
    let source = format!(
        "{}\n{}\n(func (export \"canister_update probe\")\n{}
        (global.set $access_operation (i32.const {operation}))
        {body}
        {} (call $reply)))",
        provider.trim_end().strip_suffix(')').unwrap(),
        include_str!("access.wat"),
        if claim { "(drop (call $claim))" } else { "" },
        if claim {
            "(call $settle (i64.load (i32.const 65568)) (i64.load (i32.const 65576)))"
        } else {
            ""
        },
    );
    let bytes = compile_wat(&source, 16384, 8192).unwrap();
    validate_wasm(&bytes, 8192).unwrap();
    admit(&bytes).unwrap();
    record("vector access helpers", &bytes);
    bytes
}

#[tokio::test(flavor = "current_thread")]
async fn access_helpers_bound_effective_ranges_and_refuse_uninitialized_reads() {
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;

    // Stack scratch is valid before claim, but neither side of its exact frame is.
    let stack = install(pic, candidate((3, 4), 0, false, r#"
        (call $access_store_i32 (i32.const 32688) (i32.const 123) (i64.const 0))
        (if (i32.ne (call $access_load_i32 (i32.const 32688) (i64.const 0)) (i32.const 123)) (then unreachable))
        (call $access_store_u8 (i32.const 32767) (i32.const 255) (i64.const 0))
        (if (i32.ne (call $access_load_u8 (i32.const 32767) (i64.const 0)) (i32.const 255)) (then unreachable))
        (call $access_store_i64 (i32.const 32760) (i64.const 456) (i64.const 0))
        (if (i64.ne (call $access_load_i64 (i32.const 32760) (i64.const 0)) (i64.const 456)) (then unreachable))
    "#)).await;
    let before = observe(pic, stack).await;
    update(pic, stack, "probe").await;
    assert_eq!(observe(pic, stack).await, before);

    let payload = install(pic, candidate((3, 4), 0, true, r#"
        (call $access_store_i64 (i32.const 131072) (i64.const 97) (i64.const 8))
        (if (i64.ne (call $access_load_i64 (i32.const 131072) (i64.const 8)) (i64.const 97)) (then unreachable))
    "#)).await;
    update(pic, payload, "probe").await;
    assert_eq!(observe(pic, payload).await.values(), [17, 97, 41]);

    let overlap = install(
        pic,
        candidate(
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 16))",
        ),
    )
    .await;
    update(pic, overlap, "probe").await;
    assert_eq!(observe(pic, overlap).await.values(), [23, 41, 41]);

    // Appending may initialize exactly one new word, but the read window remains
    // the admitted old prefix until the owning caller returns and settles.
    let append = install(
        pic,
        candidate(
            (3, 4),
            1,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 97) (i64.const 24))",
        ),
    )
    .await;
    update(pic, append, "probe").await;
    assert_eq!(
        &observe(pic, append).await.payload[24..32],
        &97_u64.to_le_bytes()
    );

    // Error and forget operations may read/mutate existing words. Only the error
    // path can initialize one extra word, and full capacity still allows edits.
    for (root, operation, offset) in [((3, 4), 6, 24), ((512, 512), 6, 4088), ((3, 4), 7, 16)] {
        let body = format!(
            "(call $access_store_i64 (i32.const 131072) (i64.const 97) (i64.const {offset}))"
        );
        let id = install(pic, candidate(root, operation, true, &body)).await;
        update(pic, id, "probe").await;
        assert_eq!(
            &observe(pic, id).await.payload[offset..offset + 8],
            &97u64.to_le_bytes()
        );
    }

    for (root, operation, body) in [
        (
            (512, 512),
            0,
            "(if (i64.ne (call $access_load_i64 (i32.const 131072) (i64.const 4088)) (i64.const 0)) (then unreachable))",
        ),
        (
            (1, 1),
            2,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 0))",
        ),
    ] {
        let id = install(pic, candidate(root, operation, true, body)).await;
        let before = observe(pic, id).await;
        update(pic, id, "probe").await;
        assert_eq!(observe(pic, id).await, before);
    }
    let replacement = install(
        pic,
        candidate(
            (0, 0),
            5,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 123) (i64.const 0))",
        ),
    )
    .await;
    update(pic, replacement, "probe").await;
    assert_eq!(
        &observe(pic, replacement).await.payload[..8],
        &123_u64.to_le_bytes()
    );

    for (root, operation, claim, body) in [
        (
            (3, 4),
            3,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            4,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            5,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i64 (i32.const 131073) (i64.const 4294967295)))",
        ),
        (
            (3, 4),
            0,
            false,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 65536) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 32687) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(call $access_store_i32 (i32.const 32765) (i32.const 1) (i64.const 0))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_u8 (i32.const 131071) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 17)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_u8 (i32.const 131072) (i64.const 24)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 24))",
        ),
        (
            (3, 4),
            1,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 32))",
        ),
        (
            (3, 4),
            1,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 24)) (drop (call $access_load_i64 (i32.const 131072) (i64.const 24)))",
        ),
        (
            (512, 512),
            1,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 4096))",
        ),
        (
            (0, 0),
            0,
            true,
            "(drop (call $access_load_u8 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            3,
            true,
            "(call $access_store_u8 (i32.const 131072) (i32.const 1) (i64.const 0))",
        ),
        (
            (3, 4),
            4,
            true,
            "(call $access_store_i32 (i32.const 131072) (i32.const 1) (i64.const 0))",
        ),
        (
            (3, 4),
            5,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 8))",
        ),
        (
            (3, 4),
            8,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            6,
            true,
            "(drop (call $access_load_i64 (i32.const 131096) (i64.const 0)))",
        ),
        (
            (3, 4),
            6,
            true,
            "(call $access_store_i64 (i32.const 131104) (i64.const 1) (i64.const 0))",
        ),
        (
            (512, 512),
            6,
            true,
            "(call $access_store_i64 (i32.const 135168) (i64.const 1) (i64.const 0))",
        ),
        (
            (0, 0),
            7,
            true,
            "(call $access_store_i64 (i32.const 131072) (i64.const 1) (i64.const 0))",
        ),
        (
            (3, 4),
            7,
            true,
            "(drop (call $access_load_i64 (i32.const 131096) (i64.const 0)))",
        ),
        (
            (3, 4),
            7,
            true,
            "(call $access_store_i64 (i32.const 131096) (i64.const 1) (i64.const 0))",
        ),
        (
            (3, 4),
            6,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 8))",
        ),
        (
            (3, 4),
            7,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 8))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const -1)))",
        ),
        (
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 24))",
        ),
        (
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131080) (i32.const 131072) (i32.const 24))",
        ),
        (
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 32704) (i32.const 8))",
        ),
        (
            (3, 4),
            0,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131080) (i32.const 8))",
        ),
    ] {
        let id = install(pic, candidate(root, operation, claim, body)).await;
        rejected_update(pic, id, "probe").await;
    }
    instance.drop().await;
}
