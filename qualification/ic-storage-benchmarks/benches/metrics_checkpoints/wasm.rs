//! Installed histogram checkpoint probe. Setup/reply code is excluded from timing.
#[cfg(target_arch = "wasm32")]
mod workload;
#[cfg(target_arch = "wasm32")]
mod canister {
    use super::workload::Workload;
    use ic_platform_runtime::ic0;
    use ic_storage::{DefaultMemoryImpl, Memory};
    use std::cell::RefCell;

    enum State {
        Small(Box<Workload<DefaultMemoryImpl, 1>>),
        Medium(Box<Workload<DefaultMemoryImpl, 16>>),
        Large(Box<Workload<DefaultMemoryImpl, 256>>),
    }
    thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
    fn input<const N: usize>() -> [u8; N] {
        assert_eq!(ic0::msg_arg_data_size(), N);
        let mut bytes = [0; N];
        ic0::msg_arg_data_copy(&mut bytes, 0);
        bytes
    }
    #[expect(
        unsafe_code,
        reason = "This standalone canister owns the uniquely named IC endpoint export."
    )]
    #[unsafe(export_name = "canister_init")]
    extern "C" fn init() {
        let args = input::<16>();
        let bounds = u64::from_le_bytes(args[..8].try_into().unwrap());
        let rows = usize::try_from(u64::from_le_bytes(args[8..].try_into().unwrap())).unwrap();
        assert!(matches!(rows, 1 | 64));
        let memory = DefaultMemoryImpl::default();
        let state = match bounds {
            1 => State::Small(Box::new(Workload::new(memory, rows))),
            16 => State::Medium(Box::new(Workload::new(memory, rows))),
            256 => State::Large(Box::new(Workload::new(memory, rows))),
            _ => panic!("unsupported histogram bounds"),
        };
        STATE.with(|slot| *slot.borrow_mut() = Some(state));
    }
    #[expect(
        unsafe_code,
        reason = "This standalone canister owns the uniquely named IC endpoint export."
    )]
    #[unsafe(export_name = "canister_update run")]
    extern "C" fn run() {
        let [operation, repetitions] = input::<2>();
        assert!(usize::from(operation) < super::workload::OPERATIONS.len());
        assert!(matches!(repetitions, 1 | 2));
        assert!(repetitions == 1 || operation <= 3);
        let mut costs = [0_u64; 3];
        STATE.with(|slot| {
            for cost in costs.iter_mut().take(usize::from(repetitions)) {
                let clock = || ic0::performance_counter(0);
                *cost = match slot.borrow_mut().as_mut().unwrap() {
                    State::Small(state) => state.step(operation.into(), clock),
                    State::Medium(state) => state.step(operation.into(), clock),
                    State::Large(state) => state.step(operation.into(), clock),
                };
            }
        });
        costs[2] = DefaultMemoryImpl::default().size();
        ic0::msg_reply_data_append(
            &costs
                .into_iter()
                .flat_map(u64::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        ic0::msg_reply();
    }
}
