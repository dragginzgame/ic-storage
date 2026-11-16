//! Independent logical/capacity model across mixed retained and portable histories.
use super::*;
use std::collections::VecDeque;

struct Model {
    records: VecDeque<(u32, u64)>,
    capacity: usize,
}

impl Model {
    fn initial(root: (u64, u64)) -> Self {
        Self {
            records: (0..root.0).map(|i| (17 + i as u32, 1000)).collect(),
            capacity: root.1 as usize,
        }
    }

    // Closed Rust 1.100.0 fixture uses amortized reserve for 16-byte elements.
    // Unlike the scalar core's exact reserve, growth can refuse before length 16.
    fn append_capacity(&self) -> Option<usize> {
        let next = if self.records.len() < self.capacity {
            self.capacity
        } else {
            (2 * self.capacity).max(4).max(self.records.len() + 1)
        };
        (next <= 16).then_some(next)
    }

    fn apply(&mut self, operation: usize) {
        match operation {
            0 | 6 | 7 => {
                if let Some((counter, ceiling)) = self.records.front_mut() {
                    *counter += 1;
                    assert!(u64::from(*counter) <= *ceiling);
                }
            }
            2 => {
                self.records.pop_front();
            }
            3 => self.capacity = self.records.len(),
            4 | 5 => {
                self.records.clear();
                self.capacity = 0;
            }
            _ => {}
        }
        if matches!(operation, 1 | 5 | 6)
            && let Some(capacity) = self.append_capacity()
        {
            self.records.push_back((1, 1000));
            self.capacity = capacity;
        }
    }

    fn check(&self, observation: &[u8], domain: u64, step: usize) {
        let word = |offset| u64::from_le_bytes(observation[offset..offset + 8].try_into().unwrap());
        assert_eq!(word(0), 1, "executable version at step {step}");
        assert_eq!(word(8), 1, "custody at step {step}");
        assert_eq!(word(16), 0x4e5650524f563035, "job identity at step {step}");
        assert_eq!(word(32), domain, "allocation domain at step {step}");
        assert_eq!(word(40), 256, "extent at step {step}");
        assert_eq!(word(48), self.records.len() as u64, "length at step {step}");
        assert_eq!(word(56), self.capacity as u64, "capacity at step {step}");
        assert_eq!(word(64), 1, "settlement at step {step}");
        assert_eq!(
            &observation[72..160],
            args(&[
                u64::from_le_bytes(*b"RECU64V1"),
                3,
                16,
                8,
                2,
                1,
                0,
                4,
                2,
                8,
                8
            ]),
            "record schema at step {step}"
        );
        assert_eq!(
            values(observation),
            self.records.iter().copied().collect::<Vec<_>>(),
            "records at step {step}"
        );
    }

    async fn export(&self, pic: &PocketIc, id: CanisterId, epoch: u64, source: u64, target: u64) {
        assert_eq!(
            publish(pic, id, epoch, source, target).await,
            self.records.iter().copied().collect::<Vec<_>>()
        );
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-job-test supplies fresh artifacts and prepared server"]
async fn mixed_record_histories_preserve_fields_custody_and_durable_job_state() {
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .unwrap()
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let ready = [
        candidate(false, (0, 0), Post::Reopen),
        candidate(true, (0, 0), Post::Reopen),
    ];
    let frozen = [
        candidate(false, (0, 0), Post::Frozen),
        candidate(true, (0, 0), Post::Frozen),
    ];
    let recover = [
        candidate(false, (0, 0), Post::Recover),
        candidate(true, (0, 0), Post::Recover),
    ];
    let stage = [
        candidate(false, (0, 0), Post::Stage),
        candidate(true, (0, 0), Post::Stage),
    ];
    let resume = [
        candidate(false, (0, 0), Post::Importing),
        candidate(true, (0, 0), Post::Importing),
    ];
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
    for (root, seed) in [((0, 8), 17u64), ((3, 3), 41), ((9, 9), 73), ((16, 16), 97)] {
        let id = install(pic, root).await;
        let mut model = Model::initial(root);
        let mut random = seed;
        let mut side = 0;
        let mut epoch = 0;
        let mut covered = [false; 10];
        for step in 0..96 {
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
            let domain = [A, B][side];
            let destination = [A, B][1 - side];
            model.check(&query(pic, id, "observe").await, domain, step);
            let stable = pic.get_stable_memory(id).await;
            if operation >= 8 || (operation == 1 && model.append_capacity().is_none()) {
                refuse(pic, id, id, methods[operation], &[]).await;
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
                "ordinary call changed durable job at step {step}"
            );

            if step % 24 == 7 {
                epoch += 1;
                model.export(pic, id, epoch, domain, destination).await;
                upgrade(pic, id, &frozen[side], &[epoch, destination, 0], true).await;
                refuse(pic, id, id, "cancel_checkpoint", &[epoch, destination, 2]).await;
                call(pic, id, "cancel_checkpoint", &[epoch, destination, 0]).await;
                envelope(
                    &query(pic, id, "checkpoint_bytes").await,
                    3,
                    epoch,
                    domain,
                    destination,
                    None,
                );
                // A cancellation retry must not erase writes made after thaw.
                update(pic, id, "vector_mutate").await;
                model.apply(0);
                call(pic, id, "cancel_checkpoint", &[epoch, destination, 0]).await;
                model.check(&query(pic, id, "observe").await, domain, step);
                refuse_upgrade(pic, id, &recover[1 - side], &[epoch, domain, 0], false).await;
            }
            if step % 24 == 11 {
                let before = query(pic, id, "observe").await;
                let stable = pic.get_stable_memory(id).await;
                upgrade(pic, id, &ready[side], &[], true).await;
                assert_eq!(query(pic, id, "observe").await, before);
                assert_eq!(pic.get_stable_memory(id).await, stable);
            }
            if step % 24 == 23 {
                // Ensure every staged cutover spans multiple batches/interruption,
                // even when preceding randomized operations emptied the vector.
                if step / 24 % 2 == 1 {
                    while model.records.len() < 4 {
                        update(pic, id, "vector_append").await;
                        model.apply(1);
                        model.check(&query(pic, id, "observe").await, domain, step);
                    }
                }
                epoch += 1;
                model.export(pic, id, epoch, domain, destination).await;
                if step / 24 % 2 == 0 {
                    refuse_upgrade(pic, id, &recover[1 - side], &[epoch, domain, 3], false).await;
                    upgrade(pic, id, &recover[1 - side], &[epoch, domain, 0], false).await;
                    envelope(
                        &query(pic, id, "checkpoint_bytes").await,
                        2,
                        epoch,
                        domain,
                        destination,
                        None,
                    );
                } else {
                    upgrade(pic, id, &stage[1 - side], &[epoch, domain, 0], false).await;
                    let mut cursor = 0;
                    loop {
                        let limit = if cursor % 2 == 0 { 1 } else { 3 };
                        refuse(
                            pic,
                            id,
                            id,
                            "import_batch",
                            &[epoch, domain, cursor, limit, 3],
                        )
                        .await;
                        call(pic, id, "import_batch", &[epoch, domain, cursor, limit, 0]).await;
                        let end = (cursor + limit).min(model.records.len() as u64);
                        let complete = end == model.records.len() as u64;
                        envelope(
                            &query(pic, id, "checkpoint_bytes").await,
                            if complete { 2 } else { 4 },
                            epoch,
                            domain,
                            destination,
                            Some([end as u32, cursor as u32, limit as u32]),
                        );
                        let before = query(pic, id, "observe").await;
                        let stable = pic.get_stable_memory(id).await;
                        call(pic, id, "import_batch", &[epoch, domain, cursor, limit, 0]).await;
                        assert_eq!(query(pic, id, "observe").await, before);
                        assert_eq!(pic.get_stable_memory(id).await, stable);
                        if complete {
                            break;
                        }
                        refuse(pic, id, id, "vector_mutate", &[]).await;
                        upgrade(pic, id, &resume[1 - side], &[epoch, domain, 0], true).await;
                        assert_eq!(query(pic, id, "observe").await, before);
                        assert_eq!(pic.get_stable_memory(id).await, stable);
                        cursor = end;
                    }
                }
                side = 1 - side;
                model.capacity = model.records.len();
                model.check(&query(pic, id, "observe").await, destination, step);
                assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
                // Completed and cancelled epochs remain retired throughout later histories.
                for stale in [epoch - 1, epoch] {
                    refuse_upgrade(pic, id, &recover[side], &[stale, domain, 0], false).await;
                }
            }
        }
        assert!(covered.into_iter().all(|seen| seen));
        assert_eq!((side, epoch), (0, 8));
        update(pic, id, "vector_reuse").await;
        model.apply(5);
        model.check(&query(pic, id, "observe").await, A, 96);
        println!(
            "record history seed={seed} initial={root:?}: 96 mixed operations, 4 cancellations, 2 atomic and 2 staged cutovers; all 10 operations covered"
        );
    }
    owner.close().await;
}
