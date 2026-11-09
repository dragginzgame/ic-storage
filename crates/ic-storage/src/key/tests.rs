use super::*;

#[test]
fn parse_stores_the_string_it_validates() {
    struct ChangingKey(std::cell::Cell<bool>);

    impl AsRef<str> for ChangingKey {
        fn as_ref(&self) -> &str {
            if self.0.replace(true) {
                "INVALID"
            } else {
                "app.rows.v1"
            }
        }
    }

    let key = StableKey::parse(ChangingKey(std::cell::Cell::new(false))).unwrap();
    assert_eq!(key.as_str(), "app.rows.v1");
}

#[test]
fn accepts_canonical_keys() {
    assert_eq!(
        StableKey::parse("app.users.primary.v1")
            .expect("valid key")
            .as_str(),
        "app.users.primary.v1"
    );
    assert!(StableKey::parse("framework.core.auth_state.v12").is_ok());
    assert!(StableKey::parse("app.vault.v1").is_ok());
    assert!(StableKey::parse("app.v1.v2").is_ok());
}

#[test]
fn key_decode_enforces_grammar_and_preserves_current_encoding() {
    let key = StableKey::parse("app.users.v1").unwrap();
    let json = serde_json::to_string(&key).unwrap();
    assert_eq!(json, "\"app.users.v1\"");
    assert_eq!(serde_json::from_str::<StableKey>(&json).unwrap(), key);
    let bytes = ic_wire::native::encode(
        &key,
        ic_wire::Limits {
            bytes: 1024,
            allocation: 4096,
            nodes: 64,
            depth: 8,
        },
    )
    .unwrap();
    assert_eq!(bytes, b"\x0capp.users.v1");
    assert_eq!(
        ic_wire::native::decode::<StableKey>(
            &bytes,
            ic_wire::Limits {
                bytes: 1024,
                allocation: 4096,
                nodes: 64,
                depth: 8
            }
        )
        .unwrap(),
        key
    );

    for invalid in [
        "App.users.v1",
        "app.users.v0",
        "app.vault",
        "app.users.v1.tail",
        "app..users.v1",
        &"x".repeat(129),
    ] {
        let json = serde_json::to_string(invalid).unwrap();
        assert!(serde_json::from_str::<StableKey>(&json).is_err());
        let bytes = ic_wire::native::encode(
            &invalid,
            ic_wire::Limits {
                bytes: 1024,
                allocation: 4096,
                nodes: 64,
                depth: 8,
            },
        )
        .unwrap();
        assert!(
            ic_wire::native::decode::<StableKey>(
                &bytes,
                ic_wire::Limits {
                    bytes: 1024,
                    allocation: 4096,
                    nodes: 64,
                    depth: 8
                }
            )
            .is_err()
        );
    }
}

#[test]
fn whitespace_rejection_preserves_input_and_precedence() {
    for whitespace in [' ', '\t', '\n', '\u{b}', '\u{c}', '\r'] {
        let key = format!("app.us{whitespace}ers.v1");
        assert_eq!(
            StableKey::parse(&key),
            Err(StableKeyError {
                stable_key: key,
                reason: "must not contain whitespace",
            })
        );
    }
    for (key, reason) in [
        ("app.us\u{a0}ers.v1", "must be ASCII"),
        ("App.us\u{b}ers.v1", "must be lowercase"),
    ] {
        assert_eq!(StableKey::parse(key).unwrap_err().reason, reason);
    }
}

#[test]
fn rejects_noncanonical_keys() {
    for key in [
        "",
        "App.users.v1",
        "app.users",
        "app.users.v0",
        "app..users.v1",
        ".app.users.v1",
        "app.users.v1.",
        "app-users.v1",
        "app/users.v1",
        "app.1users.v1",
    ] {
        assert!(StableKey::parse(key).is_err(), "{key} should fail");
    }
}
