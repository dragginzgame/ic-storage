//! Focused installed Logging checkpoints using the fixture's shared Wire contract.
use ic_storage_test_fixtures::logging as protocol;

use ic_testkit::pocket_ic::{CanisterId, PocketIc};
use ic_wire::native;
use protocol::{Append, Outcome, Reply, Snapshot, Update, Upgrade};
use serde::Serialize;
use std::{env, fs, time::Duration};

fn frame(id: u64, value: &impl Serialize) -> Vec<u8> {
    native::encode_request(id, value, protocol::REQUEST).unwrap()
}
async fn read(pic: &PocketIc, canister: CanisterId) -> Snapshot {
    query(pic, canister, None).await.snapshot
}
async fn query(pic: &PocketIc, canister: CanisterId, input: Option<Append>) -> Reply {
    let bytes = pic
        .query_call(
            canister,
            canister,
            "logging_query",
            frame(protocol::QUERY_ID, &input),
        )
        .await
        .unwrap();
    native::decode_response(protocol::QUERY_ID, &bytes, protocol::RESPONSE).unwrap()
}
async fn update(pic: &PocketIc, canister: CanisterId, input: &Update) -> Reply {
    let bytes = pic
        .update_call(
            canister,
            canister,
            "logging_update",
            frame(protocol::UPDATE_ID, input),
        )
        .await
        .unwrap();
    native::decode_response(protocol::UPDATE_ID, &bytes, protocol::RESPONSE).unwrap()
}
fn append(time: u64, trap: bool) -> Update {
    Update::Append(Append {
        created_at_secs: time,
        message: "retained event".into(),
        trap_after_save: trap,
    })
}

pub(super) async fn qualify(pic: &PocketIc) {
    let wasm =
        fs::read(env::var("IC_LOGGING_QUALIFICATION_WASM").expect("logging fixture path")).unwrap();
    let canister = super::install_fixture(pic, &wasm, frame(protocol::INIT_ID, &())).await;
    let initial = read(pic, canister).await;
    assert!(initial.rows.is_empty());
    assert_eq!(initial.next_sequence, Some(0));
    assert_eq!(initial.last_created_at_secs, None);

    let ephemeral = query(
        pic,
        canister,
        Some(Append {
            created_at_secs: 10,
            message: "é🦀".repeat(20),
            trap_after_save: false,
        }),
    )
    .await;
    assert_eq!(ephemeral.outcome, Outcome::Appended(0));
    assert!(ephemeral.snapshot.rows[0].truncated);
    assert_eq!(
        read(pic, canister).await,
        initial,
        "query checkpoint must not persist"
    );

    let first = update(pic, canister, &append(20, false)).await;
    assert_eq!(first.outcome, Outcome::Appended(0));
    let retained = first.snapshot;
    super::assert_trap(
        pic.update_call(
            canister,
            canister,
            "logging_update",
            frame(protocol::UPDATE_ID, &append(21, true)),
        )
        .await
        .unwrap_err(),
    );
    assert_eq!(
        read(pic, canister).await,
        retained,
        "trapped update must roll back heap and checkpoint"
    );

    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    super::assert_trap(
        pic.upgrade_canister(
            canister,
            wasm.clone(),
            frame(protocol::UPGRADE_ID, &Upgrade::TrapAfterCheckpoint),
            None,
        )
        .await
        .unwrap_err(),
    );
    assert_eq!(
        read(pic, canister).await,
        retained,
        "trapped upgrade must roll back its checkpoint"
    );
    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    pic.upgrade_canister(
        canister,
        wasm.clone(),
        frame(protocol::UPGRADE_ID, &Upgrade::Restore),
        None,
    )
    .await
    .unwrap();
    assert_eq!(read(pic, canister).await, retained);
    assert_eq!(
        update(pic, canister, &append(19, false)).await.outcome,
        Outcome::TimestampRefused
    );
    assert_eq!(read(pic, canister).await, retained);
    assert_eq!(
        update(pic, canister, &append(20, false)).await.outcome,
        Outcome::Appended(1)
    );
    let pruned = update(
        pic,
        canister,
        &Update::Prune {
            now_secs: 21,
            max_age_secs: 0,
            max_remove: 4,
        },
    )
    .await;
    assert_eq!(pruned.outcome, Outcome::Pruned(2));
    assert!(pruned.snapshot.rows.is_empty());
    assert_eq!(pruned.snapshot.next_sequence, Some(2));
    assert_eq!(pruned.snapshot.last_created_at_secs, Some(20));
    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    pic.upgrade_canister(
        canister,
        wasm,
        frame(protocol::UPGRADE_ID, &Upgrade::Restore),
        None,
    )
    .await
    .unwrap();
    assert_eq!(read(pic, canister).await, pruned.snapshot);
    assert_eq!(
        update(pic, canister, &append(19, false)).await.outcome,
        Outcome::TimestampRefused
    );
    let resumed = update(pic, canister, &append(20, false)).await;
    assert_eq!(resumed.outcome, Outcome::Appended(2));
    assert_eq!(resumed.snapshot.next_sequence, Some(3));
    let wrong_identity = frame(protocol::UPDATE_ID, &None::<Append>);
    super::assert_trap(
        pic.query_call(canister, canister, "logging_query", wrong_identity)
            .await
            .unwrap_err(),
    );
    assert_eq!(read(pic, canister).await, resumed.snapshot);
    let budget = update(pic, canister, &Update::TextBudgetProbe).await;
    assert_eq!(budget.outcome, Outcome::BudgetChecked);
    assert_eq!(
        budget
            .snapshot
            .rows
            .iter()
            .map(|row| row.sequence)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(budget.snapshot.next_sequence, Some(4));
    assert_eq!(read(pic, canister).await, resumed.snapshot);
    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    pic.upgrade_canister(
        canister,
        fs::read(env::var("IC_LOGGING_QUALIFICATION_WASM").unwrap()).unwrap(),
        frame(protocol::UPGRADE_ID, &Upgrade::Restore),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        update(pic, canister, &Update::ReadTextBudget)
            .await
            .snapshot,
        budget.snapshot
    );
    assert_eq!(read(pic, canister).await, resumed.snapshot);
    println!(
        "installed logging text budget: multi-row eviction, append refusal, forged retained ceiling and upgrade reopen pass"
    );
    println!(
        "installed logging: query non-persistence, update/upgrade rollback, reopen and empty-history watermarks pass"
    );
}

#[cfg(test)]
mod tests;
