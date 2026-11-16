//! Independent logical oracle across repeated native use and portable cutovers.
use super::*;
use ic_storage_retention_probe::artifact::sha256_hex;
use std::collections::VecDeque;

struct Model {
    values: VecDeque<u64>,
    capacity: usize,
}

impl Model {
    fn initial(length: usize, capacity: usize) -> Self {
        Self {
            values: (0..length)
                .map(|index| [17, 23, 41].get(index).copied().unwrap_or(0))
                .collect(),
            capacity,
        }
    }

    fn apply(&mut self, operation: usize) {
        match operation {
            0 | 6 | 7 => {
                if let Some(first) = self.values.front_mut() {
                    *first = first.wrapping_add(1);
                }
            }
            2 => {
                self.values.pop_front();
            }
            3 => self.capacity = self.values.len(),
            4 | 5 => {
                self.values.clear();
                self.capacity = 0;
            }
            _ => {}
        }
        if matches!(operation, 1 | 5 | 6) && self.values.len() < 512 {
            self.values.push_back(1);
            self.capacity = self.capacity.max(self.values.len());
        }
    }

    fn check(&self, observation: &[u8], domain: u64, step: usize) {
        let header = words(&observation[..72]);
        assert_eq!(header[0], 1, "version at step {step}");
        assert_eq!(header[1], 1, "custody at step {step}");
        assert_eq!(
            &header[4..9],
            &[
                domain,
                4096,
                self.values.len() as u64,
                self.capacity as u64,
                1
            ],
            "root at step {step}"
        );
        assert_eq!(
            logical(observation),
            self.values.iter().copied().collect::<Vec<_>>(),
            "values at step {step}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies fresh compiled inputs and the prepared server"]
async fn mixed_native_histories_match_a_model_across_repeated_domain_migrations() {
    let (directory, raw, bridge) = job_inputs();
    let save = |relocated, root, recovery| {
        let bytes = prepare_job(&raw, &bridge, relocated, root, recovery);
        let digest = sha256_hex(&bytes);
        fs::write(directory.join(format!("history-{digest}.wasm")), &bytes).unwrap();
        println!(
            "history relocated={relocated} root={root:?} recovery={recovery} bytes={} sha256={digest}",
            bytes.len()
        );
        bytes
    };
    let ready = [save(false, (0, 0), false), save(true, (0, 0), false)];
    let recovery = [save(false, (0, 0), true), save(true, (0, 0), true)];
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let methods = [
        "vector_mutate",
        "vector_append",
        "vector_remove",
        "vector_shrink",
        "vector_clear",
        "vector_reuse",
        "vector_business_error",
        "vector_forget",
        "vector_error_trap",
        "vector_error_finalize_trap",
    ];
    for (root, seed) in [((0, 8), 17u64), ((3, 3), 41), ((512, 512), 97)] {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(id, save(false, root, false), vec![], None)
            .await;
        let mut model = Model::initial(root.0 as usize, root.1 as usize);
        let mut random = seed;
        let mut side = 0;
        let mut epoch = 0;
        let mut covered = [false; 10];
        for step in 0..96 {
            // First force capacity refusal, error, remove/shrink/grow, and both
            // failure boundaries. Then replay a fixed seed with every operation
            // guaranteed in each ten-step block, in a varying cyclic order.
            let operation = if step < 10 {
                [1, 6, 2, 3, 1, 8, 9, 7, 4, 5][step]
            } else {
                if step % 10 == 0 {
                    random ^= random << 13;
                    random ^= random >> 7;
                    random ^= random << 17;
                }
                (step + (random % 10) as usize) % 10
            };
            covered[operation] = true;
            let domain = [131072, 147456][side];
            let before = query(pic, id, "observe").await;
            model.check(&before, domain, step);
            let stable = pic.get_stable_memory(id).await;
            if operation >= 8 || (operation == 1 && model.values.len() == 512) {
                refused_update(pic, id, id, methods[operation], vec![]).await;
                assert_eq!(query(pic, id, "observe").await, before);
            } else {
                let response = pic
                    .update_call(id, id, methods[operation], vec![])
                    .await
                    .unwrap();
                assert_eq!(
                    response,
                    if operation == 6 {
                        1u32.to_le_bytes().to_vec()
                    } else {
                        vec![]
                    }
                );
                model.apply(operation);
            }
            model.check(&query(pic, id, "observe").await, domain, step);
            assert_eq!(
                pic.get_stable_memory(id).await,
                stable,
                "ordinary call changed checkpoint at step {step}"
            );

            if step % 24 == 11 {
                let before = query(pic, id, "observe").await;
                pic.upgrade_eop_canister(id, ready[side].clone(), vec![], None)
                    .await
                    .unwrap();
                assert_eq!(query(pic, id, "observe").await, before);
                assert_eq!(pic.get_stable_memory(id).await, stable);
            }
            if step % 24 == 23 {
                epoch += 1;
                let destination = [131072, 147456][1 - side];
                let bytes = publish(pic, id, epoch, domain, destination).await;
                let checkpoint = envelope(&bytes, 1, epoch, domain, destination);
                assert_eq!(&checkpoint[56..64], &41u64.to_le_bytes());
                // Failed reconstruction preserves the frozen source and job.
                refused_import(pic, id, &recovery[1 - side], args(epoch, domain, 3)).await;
                import(pic, id, &recovery[1 - side], epoch, domain, destination).await;
                side = 1 - side;
                // Portable reconstruction retains logical values, not spare capacity.
                model.capacity = model.values.len();
                model.check(&query(pic, id, "observe").await, destination, step);
            }
        }
        assert!(covered.into_iter().all(|seen| seen));
        assert_eq!(side, 0);
        // The last reconstructed root must support fresh allocation too.
        update(pic, id, "vector_reuse").await;
        model.apply(5);
        model.check(&query(pic, id, "observe").await, 131072, 96);
    }
    instance.drop().await;
}
