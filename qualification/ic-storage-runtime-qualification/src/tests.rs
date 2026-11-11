use super::*;

#[test]
#[should_panic(expected = "expected an execution trap")]
fn missing_canister_cannot_qualify_as_rollback_trap() {
    assert_trap(RejectResponse {
        reject_code: ic_testkit::pocket_ic::RejectCode::DestinationInvalid,
        reject_message: "missing canister".into(),
        error_code: ErrorCode::CanisterNotFound,
        certified: false,
    });
}

#[test]
fn memory_wire_frames_admit_extremes_and_refuse_identity_trailing_and_budget_errors() {
    use ic_wire::native;
    use memory::protocol;
    let request = protocol::IoRequest {
        operation: 2,
        count: 1,
        offset: u64::MAX,
    };
    let bytes = memory::frame(protocol::IO_ID, &request);
    assert_eq!(
        native::decode_request::<protocol::IoRequest>(protocol::IO_ID, &bytes, protocol::REQUEST)
            .unwrap(),
        request
    );
    assert!(
        native::decode_request::<protocol::IoRequest>(
            protocol::SNAPSHOT_ID,
            &bytes,
            protocol::REQUEST
        )
        .is_err()
    );
    for end in 0..bytes.len() {
        assert!(
            native::decode_request::<protocol::IoRequest>(
                protocol::IO_ID,
                &bytes[..end],
                protocol::REQUEST
            )
            .is_err()
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        native::decode_request::<protocol::IoRequest>(
            protocol::IO_ID,
            &trailing,
            protocol::REQUEST
        )
        .is_err()
    );
    let oversized = vec![0; protocol::REQUEST.bytes + 1];
    assert!(
        native::decode_request::<protocol::IoRequest>(
            protocol::IO_ID,
            &oversized,
            protocol::REQUEST
        )
        .is_err()
    );
    let snapshot = protocol::Snapshot {
        generation: u64::MAX,
        markers: [0x42, 0x6d],
        physical_pages: 9,
        rows_pages: 1,
    };
    let response =
        native::encode_response(protocol::SNAPSHOT_ID, &snapshot, protocol::RESPONSE).unwrap();
    assert_eq!(
        native::decode_response::<protocol::Snapshot>(
            protocol::SNAPSHOT_ID,
            &response,
            protocol::RESPONSE
        )
        .unwrap(),
        snapshot
    );
    let empty = memory::frame(protocol::INIT_ID, &());
    assert!(native::decode_request::<()>(protocol::INIT_ID, &empty, protocol::REQUEST).is_ok());
}

#[test]
fn measurement_arguments_reject_extra_tokens() {
    assert!(
        Arguments::try_parse_from(["runner", "--storage-only"])
            .unwrap()
            .storage_only
    );
    for other in ["--logging-only", "--measure-valid-io"] {
        assert!(Arguments::try_parse_from(["runner", "--storage-only", other]).is_err());
    }
    assert!(
        Arguments::try_parse_from(["runner", "--logging-only"])
            .unwrap()
            .logging_only
    );
    assert!(Arguments::try_parse_from(["runner", "--logging-only", "--measure-valid-io"]).is_err());
    assert!(
        !Arguments::try_parse_from(["runner"])
            .unwrap()
            .measure_valid_io
    );
    assert!(
        Arguments::try_parse_from(["runner", "--measure-valid-io"])
            .unwrap()
            .measure_valid_io
    );
    for args in [
        vec!["runner", "unexpected"],
        vec!["runner", "--measure-valid-io", "unexpected"],
        vec!["runner", "--unknown"],
    ] {
        assert!(Arguments::try_parse_from(args).is_err());
    }
}
