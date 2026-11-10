//! Installed IO/lifecycle qualification fixture, not an application integration API.
//! Shared bounded Wire messages avoid local packet encoding.

#[cfg(target_arch = "wasm32")]
pub mod support;
use ic_storage_test_fixtures::io as protocol;
#[cfg(target_arch = "wasm32")]
use support::host;

#[cfg(target_arch = "wasm32")]
#[expect(
    unsafe_code,
    reason = "test-only raw IC system calls and Memory::read_unsafe"
)]
mod canister {
    use super::host::RUNTIME;
    use super::{pool, protocol};
    use ic_storage::{GenericAllocationPolicy, Memory, StorageConfig};
    use ic_wire::native;
    use serde::{Serialize, de::DeserializeOwned};

    fn with_runtime<T>(operation: impl FnOnce(&super::host::Runtime) -> T) -> T {
        RUNTIME.with(|slot| operation(slot.borrow().as_ref().expect("initialized fixture runtime")))
    }

    fn open_memory(
        key: &str,
    ) -> Result<
        ic_storage::RuntimeMemory<ic_storage::DefaultMemoryImpl>,
        ic_storage::RuntimeOpenError,
    > {
        with_runtime(|runtime| runtime.open_memory(&ic_storage::StableKey::parse(key).unwrap()))
    }

    const PAGE: u64 = 65_536;
    const ROWS: &str = "io_qualification.rows.v1";
    const NEIGHBOR: &str = "io_qualification.neighbor.v1";

    fn declarations() -> ic_storage::SealedDeclarationSnapshot {
        ic_storage::SealedDeclarationSnapshot::new(vec![
            ic_storage::MemoryRequest::new("io_qualification", "io_qualification.neighbor.v1")
                .unwrap(),
            ic_storage::MemoryRequest::new("io_qualification", "io_qualification.rows.v1").unwrap(),
        ])
        .unwrap()
    }

    #[link(wasm_import_module = "ic0")]
    unsafe extern "C" {
        fn msg_arg_data_size() -> usize;
        fn msg_arg_data_copy(dst: usize, offset: usize, size: usize);
        fn msg_reply_data_append(src: usize, size: usize);
        fn msg_reply();
        fn trap(src: usize, size: usize) -> !;
        fn performance_counter(counter_type: u32) -> u64;
    }

    fn arguments<T: DeserializeOwned>(id: u64) -> T {
        // SAFETY: Admit the frame length before allocating/copying argument bytes.
        let bytes = unsafe {
            let size = msg_arg_data_size();
            assert!(size <= protocol::REQUEST.bytes);
            let mut bytes = vec![0; size];
            msg_arg_data_copy(bytes.as_mut_ptr() as usize, 0, size);
            bytes
        };
        native::decode_request(id, &bytes, protocol::REQUEST).unwrap()
    }

    fn reply(id: u64, value: &impl Serialize) {
        let bytes = native::encode_response(id, value, protocol::RESPONSE).unwrap();
        // SAFETY: The encoded source is live through the synchronous system copy.
        unsafe {
            msg_reply_data_append(bytes.as_ptr() as usize, bytes.len());
            msg_reply();
        }
    }

    fn bootstrap(mode: u8) {
        let config = if mode == 2 {
            StorageConfig {
                allocations: 1,
                ..StorageConfig::default()
            }
        } else {
            StorageConfig::default()
        };
        let mut runtime =
            super::host::Runtime::new_with_config(ic_storage::DefaultMemoryImpl::default(), config)
                .unwrap();
        runtime
            .bootstrap(&declarations(), &pool(), &GenericAllocationPolicy)
            .unwrap();
        RUNTIME.with(|slot| *slot.borrow_mut() = Some(runtime));
        assert_ne!(mode, 1, "injected trap after durable bootstrap write");
    }

    #[unsafe(export_name = "canister_init")]
    extern "C" fn init() {
        arguments::<()>(protocol::INIT_ID);
        bootstrap(0);
        let neighbor = open_memory(NEIGHBOR).unwrap();
        neighbor.grow(8).unwrap();
        neighbor.write(8 * PAGE - 1, &[0x6D]);
        let rows = open_memory(ROWS).unwrap();
        rows.grow(1).unwrap();
        rows.write(0, &[0x42]);
        // Infrastructure-only backing checks: IC0 reports refusal as u64::MAX.
        let backing = ic_storage::DefaultMemoryImpl::default();
        let pages = backing.size();
        assert_eq!(backing.grow(0), i64::try_from(pages).unwrap());
        assert_eq!(backing.grow(u64::MAX), -1);
        assert_eq!(backing.size(), pages);
        let mut marker = [0];
        rows.read(0, &mut marker);
        assert_eq!(marker, [0x42]);
    }

    #[unsafe(export_name = "canister_post_upgrade")]
    extern "C" fn post_upgrade() {
        bootstrap(arguments(protocol::UPGRADE_ID));
    }

    #[unsafe(export_name = "canister_update composition")]
    extern "C" fn composition() {
        arguments::<()>(protocol::COMPOSITION_ID);
        // Two independent components use one host runtime. Reconstructing the
        // default backing is refused even after that owner drops, until both
        // retained component handles have released custody.
        let runtime = RUNTIME.with(|slot| slot.borrow_mut().take().unwrap());
        let generation = runtime.committed_generation().unwrap();
        let rows = runtime
            .open_memory(&ic_storage::StableKey::parse(ROWS).unwrap())
            .unwrap();
        let neighbor = runtime
            .open_memory(&ic_storage::StableKey::parse(NEIGHBOR).unwrap())
            .unwrap();
        let physical = ic_storage::DefaultMemoryImpl::default().size();
        let refused = || {
            assert!(matches!(
                super::host::Runtime::new(ic_storage::DefaultMemoryImpl::default()),
                Err(ic_storage::RuntimeConstructionError::BackingAlreadyClaimed)
            ));
        };
        refused();
        rows.write(0, &[0x42]);
        neighbor.write(8 * PAGE - 1, &[0x6D]);
        drop(runtime);
        refused();
        let mut marker = [0];
        rows.read(0, &mut marker);
        assert_eq!(marker, [0x42]);
        drop(rows);
        refused();
        neighbor.read(8 * PAGE - 1, &mut marker);
        assert_eq!(marker, [0x6D]);
        drop(neighbor);
        bootstrap(0);
        assert_eq!(
            with_runtime(|runtime| runtime.committed_generation().unwrap()),
            generation
        );
        assert_eq!(ic_storage::DefaultMemoryImpl::default().size(), physical);
        reply(protocol::COMPOSITION_ID, &());
    }

    #[unsafe(export_name = "canister_update metadata_fault")]
    extern "C" fn metadata_fault() {
        let mode: u8 = arguments(protocol::METADATA_FAULT_ID);
        assert!(mode < 6);
        let rows = open_memory(ROWS).unwrap();
        rows.write(0, &[0xA5]);
        // Infrastructure-only fault injection over the actual canister backing.
        // The driver requires our marker, not just any assertion/engine trap.
        let backing = ic_storage::DefaultMemoryImpl::default();
        let offset = [0_u64, 8, 48][usize::from(mode % 3)];
        let mut byte = [0];
        backing.read(offset, &mut byte);
        byte[0] ^= 1;
        backing.write(offset, &byte);
        // Custody-based opening does not scan the altered backing.
        let neighbor = open_memory(NEIGHBOR).unwrap();
        let verification = if mode < 3 {
            with_runtime(|runtime| runtime.diagnostics().map(|_| ()))
        } else {
            with_runtime(|runtime| runtime.memory_allocations().map(|_| ()))
        };
        assert!(matches!(
            verification,
            Err(ic_storage::DirectoryError::Invalid(_))
        ));
        assert_eq!(rows.grow(0), Err(ic_storage::RuntimeGrowError::Poisoned));
        assert_eq!(
            neighbor.grow(1),
            Err(ic_storage::RuntimeGrowError::Poisoned)
        );
        drop(neighbor);
        assert!(matches!(
            open_memory(NEIGHBOR),
            Err(ic_storage::RuntimeOpenError::Directory(
                ic_storage::DirectoryError::Poisoned
            ))
        ));
        let marker = b"verified-directory-poisoning";
        // SAFETY: the static marker is an initialized readable range; trap never returns.
        unsafe { trap(marker.as_ptr() as usize, marker.len()) }
    }

    #[unsafe(export_name = "canister_query snapshot")]
    extern "C" fn snapshot() {
        arguments::<()>(protocol::SNAPSHOT_ID);
        let generation = with_runtime(|runtime| runtime.committed_generation().unwrap());
        // Check assigned identities independently of payload retention.
        let neighbor_id = with_runtime(|runtime| {
            runtime
                .allocation_id(&ic_storage::StableKey::parse(NEIGHBOR).unwrap())
                .unwrap()
        });
        let rows_id = with_runtime(|runtime| {
            runtime
                .allocation_id(&ic_storage::StableKey::parse(ROWS).unwrap())
                .unwrap()
        });
        assert_ne!(neighbor_id, rows_id);
        let rows = open_memory(ROWS).unwrap();
        let neighbor = open_memory(NEIGHBOR).unwrap();
        assert!(matches!(
            open_memory(ROWS),
            Err(ic_storage::RuntimeOpenError::AlreadyOpen { .. })
        ));
        let mut markers = [0; 2];
        rows.read(0, &mut markers[..1]);
        neighbor.read(8 * PAGE - 1, &mut markers[1..]);
        reply(
            protocol::SNAPSHOT_ID,
            &protocol::Snapshot {
                generation,
                markers,
                physical_pages: ic_storage::DefaultMemoryImpl::default().size(),
                rows_pages: rows.size(),
            },
        );
    }

    #[unsafe(export_name = "canister_update io")]
    extern "C" fn io() {
        let protocol::IoRequest {
            operation,
            count,
            offset,
        } = arguments(protocol::IO_ID);
        let count = usize::from(count);
        assert!(count <= 2);
        let rows = open_memory(ROWS).unwrap();
        // Warm the cache and dirty valid state before the candidate failure.
        // A trap must roll this mutation back as well as reject the invalid IO.
        rows.write(0, &[0x99]);
        let mut dst = [0; 2];
        // SAFETY: Counter zero is supported in update execution.
        let start = unsafe { performance_counter(0) };
        for _ in 0..1000 {
            match operation {
                0 => rows.read(offset, &mut dst[..count]),
                // Exercise the trait entry point independently of the inherent read.
                1 => Memory::read(&rows, offset, &mut dst[..count]),
                2 => rows.write(offset, &[0xA5; 2][..count]),
                _ => panic!("unknown IO operation"),
            }
        }
        // SAFETY: Same counter/execution as above.
        let instructions = unsafe { performance_counter(0) } - start;
        reply(
            protocol::IO_ID,
            &protocol::IoReply {
                bytes: dst,
                instructions,
            },
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn pool() -> ic_storage::AllocationGrants {
    ic_storage::AllocationGrants::new(vec![
        ic_storage::MemoryAuthority::new("io_qualification", "io_qualification.").unwrap(),
    ])
    .unwrap()
}
