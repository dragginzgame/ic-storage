use super::*;

#[test]
fn logging_wire_admits_maximum_fixture_rows_and_binds_lifecycle_identity() {
    let value = Reply {
        outcome: Outcome::Appended(u64::MAX),
        snapshot: Snapshot {
            rows: vec![
                protocol::Row {
                    sequence: u64::MAX,
                    created_at_secs: u64::MAX,
                    message: "x".repeat(64),
                    truncated: true
                };
                4
            ],
            next_sequence: None,
            last_created_at_secs: Some(u64::MAX),
        },
    };
    let bytes = native::encode_response(protocol::QUERY_ID, &value, protocol::RESPONSE).unwrap();
    assert_eq!(
        native::decode_response::<Reply>(protocol::QUERY_ID, &bytes, protocol::RESPONSE).unwrap(),
        value
    );
    let bytes = frame(protocol::UPGRADE_ID, &Upgrade::Restore);
    assert!(
        native::decode_request::<Upgrade>(protocol::INIT_ID, &bytes, protocol::REQUEST).is_err()
    );
    assert!(
        native::decode_request::<Upgrade>(protocol::UPGRADE_ID, &bytes, protocol::REQUEST).is_ok()
    );
}
