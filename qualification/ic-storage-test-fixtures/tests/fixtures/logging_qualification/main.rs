//! Explicit installed checkpoint qualification; no application endpoint is exported by the library.

#[cfg(target_arch = "wasm32")]
use ic_storage_test_fixtures::logging as protocol;

#[cfg(target_arch = "wasm32")]
mod canister {
    use super::protocol::{self, Append, Outcome, Reply, Row, Snapshot, Update, Upgrade};
    use ic_logging::{AppendError, EntryInput, Level, Limits, Log};
    use ic_logging_storage::{LogIdentity, LogKey, LoggingStore, StoreError};
    use ic_platform_runtime::cdk;
    use ic_storage::adapter::StableBTreeMap;
    use ic_storage::{DefaultMemoryImpl, MemoryRuntime, StorageConfig, testing::fixture_runtime};
    use ic_wire::native;
    use serde::{Serialize, de::DeserializeOwned};
    use std::{cell::RefCell, num::NonZeroUsize};

    const KEY: &str = "logging.fixture.v1";
    const IDENTITY: LogIdentity = LogIdentity { epoch: 7 };
    const LIMITS: Limits = Limits {
        max_entries: NonZeroUsize::new(4).unwrap(),
        max_message_bytes: 64,
        max_label_bytes: 32,
        max_text_bytes: 128,
    };

    struct Host {
        _runtime: MemoryRuntime<DefaultMemoryImpl>,
        store: LoggingStore<DefaultMemoryImpl>,
        log: Log,
    }
    thread_local! { static HOST: RefCell<Option<Host>> = const { RefCell::new(None) }; }

    fn required<T>(value: Result<T, impl std::fmt::Debug>) -> T {
        value.unwrap_or_else(|error| cdk::api::trap(format!("logging fixture refused: {error:?}")))
    }
    fn key() -> LogKey {
        required(LogKey::new("fixture", "worker"))
    }
    fn admit() -> Result<(), String> {
        if ic_platform_runtime::ic0::msg_arg_data_size() > protocol::REQUEST.bytes {
            return Err("logging fixture frame limit".into());
        }
        Ok(())
    }
    #[expect(
        clippy::needless_pass_by_value,
        reason = "CDK decoder signature owns argument bytes"
    )]
    fn decode<T: DeserializeOwned, const ID: u64>(bytes: Vec<u8>) -> T {
        required(native::decode_request(ID, &bytes, protocol::REQUEST))
    }
    fn encode<T: Serialize, const ID: u64>(value: T) -> Vec<u8> {
        required(native::encode_response(ID, &value, protocol::RESPONSE))
    }
    fn open(fresh: bool) {
        let runtime = required(fixture_runtime(
            DefaultMemoryImpl::default(),
            "logging",
            "logging.",
            &[KEY],
            StorageConfig::default(),
        ));
        let memory = required(runtime.open_memory(&ic_storage::StableKey::parse(KEY).unwrap()));
        let mut store = LoggingStore::new(if fresh {
            StableBTreeMap::create(memory).unwrap()
        } else {
            StableBTreeMap::load(memory).unwrap()
        });
        let log = if fresh {
            let log = Log::new(LIMITS);
            required(store.save(&key(), IDENTITY, &log));
            log
        } else {
            required(store.load(&key(), IDENTITY, LIMITS))
                .unwrap_or_else(|| cdk::api::trap("retained log checkpoint is absent"))
        };
        HOST.with_borrow_mut(|slot| {
            *slot = Some(Host {
                _runtime: runtime,
                store,
                log,
            });
        });
    }
    fn with_host<T>(action: impl FnOnce(&mut Host) -> T) -> T {
        HOST.with_borrow_mut(|slot| action(slot.as_mut().expect("initialized logging fixture")))
    }
    fn snapshot(log: &Log) -> Snapshot {
        let state = log.snapshot();
        Snapshot {
            next_sequence: state.next_sequence,
            last_created_at_secs: state.last_created_at_secs,
            rows: state
                .entries
                .into_iter()
                .map(|row| Row {
                    sequence: row.sequence,
                    created_at_secs: row.created_at_secs,
                    message: row.message,
                    truncated: row.truncated,
                })
                .collect(),
        }
    }
    fn append(host: &mut Host, input: &Append) -> Outcome {
        let outcome = match host.log.append(EntryInput {
            created_at_secs: input.created_at_secs,
            source: "probe",
            topic: None,
            level: Level::Info,
            message: &input.message,
        }) {
            Ok(receipt) => Outcome::Appended(receipt.sequence),
            Err(AppendError::TimestampRegressed { .. }) => return Outcome::TimestampRefused,
            Err(AppendError::LabelTooLong { .. }) => return Outcome::LabelRefused,
            Err(AppendError::SequenceExhausted) => return Outcome::Exhausted,
            Err(AppendError::TextBudgetExceeded { .. }) => return Outcome::TextRefused,
        };
        required(host.store.save(&key(), IDENTITY, &host.log));
        if input.trap_after_save {
            cdk::api::trap("logging fixture trap after checkpoint");
        }
        outcome
    }

    const TEXT_LIMITS: Limits = Limits {
        max_text_bytes: 20,
        ..LIMITS
    };
    fn text_key() -> LogKey {
        required(LogKey::new("fixture", "text_budget"))
    }

    // Exercise the real governed row, including explicit host corruption and repair.
    fn text_budget_probe(
        mut store: LoggingStore<DefaultMemoryImpl>,
    ) -> (LoggingStore<DefaultMemoryImpl>, Reply) {
        let mut log = Log::new(TEXT_LIMITS);
        for time in 0..3 {
            required(log.append(EntryInput {
                created_at_secs: time,
                source: "probe",
                topic: None,
                level: Level::Info,
                message: "x",
            }));
        }
        required(store.save(&text_key(), IDENTITY, &log));
        let receipt = required(log.append(EntryInput {
            created_at_secs: 3,
            source: "probe",
            topic: None,
            level: Level::Info,
            message: "123456789",
        }));
        assert_eq!(receipt.evicted, 2);
        assert_eq!(receipt.sequence, 3);
        assert_eq!(log.retained_text_bytes(), 20);
        required(store.save(&text_key(), IDENTITY, &log));
        let before = log.snapshot();
        assert!(matches!(
            log.append(EntryInput {
                created_at_secs: 4,
                source: "probe",
                topic: None,
                level: Level::Info,
                message: "a row that cannot fit"
            }),
            Err(AppendError::TextBudgetExceeded { .. })
        ));
        assert_eq!(log.snapshot(), before);
        let mut map = store.into_map();
        let raw_key = b"fixture\0text_budget".to_vec();
        let original = map.get(&raw_key).unwrap();
        // The frozen schema has four fixed limits after epoch. Lower only the
        // text ceiling; the retained rows then exceed it by one UTF-8 byte.
        let prefix = required(native::encode(
            &(
                IDENTITY.epoch,
                LIMITS.max_entries.get(),
                LIMITS.max_message_bytes,
                LIMITS.max_label_bytes,
            ),
            protocol::REQUEST,
        ));
        assert_eq!(&original[..4], b"ICL\x03");
        assert_eq!(&original[4..4 + prefix.len()], prefix.as_slice());
        let mut forged = original.clone();
        let offset = 4 + prefix.len();
        assert_eq!(forged[offset], 20);
        forged[offset] = 19;
        map.insert(raw_key.clone(), forged.clone());
        let store = LoggingStore::new(map);
        assert!(matches!(
            store.load(
                &text_key(),
                IDENTITY,
                Limits {
                    max_text_bytes: 19,
                    ..TEXT_LIMITS
                }
            ),
            Err(StoreError::InvalidState(
                ic_logging::RestoreError::TextBudgetExceeded
            ))
        ));
        let mut map = store.into_map();
        assert_eq!(map.get(&raw_key), Some(forged));
        map.insert(raw_key, original);
        let store = LoggingStore::new(map);
        let restored = required(store.load(&text_key(), IDENTITY, TEXT_LIMITS)).unwrap();
        assert_eq!(restored.snapshot(), before);
        let reply = Reply {
            outcome: Outcome::BudgetChecked,
            snapshot: snapshot(&restored),
        };
        (store, reply)
    }

    #[cdk::init]
    fn init() {
        required(admit());
        let _: () = decode::<_, { protocol::INIT_ID }>(cdk::api::msg_arg_data());
        open(true);
    }

    #[cdk::post_upgrade]
    fn upgrade() {
        required(admit());
        let mode: Upgrade = decode::<_, { protocol::UPGRADE_ID }>(cdk::api::msg_arg_data());
        open(false);
        if matches!(mode, Upgrade::TrapAfterCheckpoint) {
            with_host(|host| {
                let created_at_secs = host.log.snapshot().last_created_at_secs.unwrap_or(0);
                append(
                    host,
                    &Append {
                        created_at_secs,
                        message: "upgrade checkpoint".into(),
                        trap_after_save: true,
                    },
                );
            });
        }
    }

    #[cdk::query(
        guard = "admit",
        decode_with = "decode::<_, { protocol::QUERY_ID }>",
        encode_with = "encode::<_, { protocol::QUERY_ID }>"
    )]
    fn logging_query(input: Option<Append>) -> Reply {
        with_host(|host| {
            let outcome = input.map_or(Outcome::Read, |input| append(host, &input));
            Reply {
                outcome,
                snapshot: snapshot(&host.log),
            }
        })
    }

    #[cdk::update(
        guard = "admit",
        decode_with = "decode::<_, { protocol::UPDATE_ID }>",
        encode_with = "encode::<_, { protocol::UPDATE_ID }>"
    )]
    fn logging_update(input: Update) -> Reply {
        if matches!(input, Update::TextBudgetProbe) {
            return HOST.with_borrow_mut(|slot| {
                let host = slot.take().expect("initialized logging fixture");
                let (store, reply) = text_budget_probe(host.store);
                *slot = Some(Host { store, ..host });
                reply
            });
        }
        with_host(|host| {
            let outcome = match input {
                Update::Append(input) => append(host, &input),
                Update::TextBudgetProbe => {
                    unreachable!("probe was handled before borrowing the host")
                }
                Update::ReadTextBudget => {
                    let log =
                        required(host.store.load(&text_key(), IDENTITY, TEXT_LIMITS)).unwrap();
                    return Reply {
                        outcome: Outcome::Read,
                        snapshot: snapshot(&log),
                    };
                }
                Update::Prune {
                    now_secs,
                    max_age_secs,
                    max_remove,
                } => {
                    let batch =
                        host.log
                            .prune_expired(now_secs, max_age_secs, usize::from(max_remove));
                    required(host.store.save(&key(), IDENTITY, &host.log));
                    Outcome::Pruned(
                        u64::try_from(batch.removed).expect("bounded fixture row count"),
                    )
                }
            };
            Reply {
                outcome,
                snapshot: snapshot(&host.log),
            }
        })
    }
}
