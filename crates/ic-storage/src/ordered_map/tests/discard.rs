use super::*;

#[test]
fn discard_mutations_skip_old_payload_io_and_decoding() {
    for length in [0, 8, 24, 25, 256, 65_536] {
        for replacement_length in [8, 1024] {
            let memory = TestMemory::default();
            let mut map = OrderedMap::<u64, DecodeProbe, _>::new(memory.clone());
            let value = |length| DecodeProbe {
                bytes: vec![31; length],
                owned: false,
            };
            assert!(!map.insert_discard(7, value(length)));
            let node = map.store.node(map.store.root);
            let start = if node.value == 0 {
                node.address + 104
            } else {
                node.value + 16
            };
            memory.forbidden.set(Some((start, start + length as u64)));
            VALUE_DECODES.set(0);
            assert!(map.insert_discard(7, value(replacement_length)));
            assert_eq!(VALUE_DECODES.get(), 0);
            assert_eq!(map.len(), 1);
            memory.forbidden.set(None);
            let node = map.store.node(map.store.root);
            let start = if node.value == 0 {
                node.address + 104
            } else {
                node.value + 16
            };
            memory
                .forbidden
                .set(Some((start, start + replacement_length as u64)));
            assert!(map.remove_discard(&7));
            assert!(!map.remove_discard(&7));
            assert_eq!(VALUE_DECODES.get(), 0);
            assert_eq!(map.len(), 0);
            memory.forbidden.set(None);
            assert!(!map.insert_discard(9, value(length)));
            let map = OrderedMap::<u64, DecodeProbe, _>::load(memory).unwrap();
            assert_eq!(map.get(&9).unwrap().bytes, vec![31; length]);
        }
    }
}

#[test]
fn discard_mutations_reject_invalid_value_blocks_before_writes() {
    for remove in [false, true] {
        for corruption in 0..3 {
            let memory = TestMemory::default();
            let mut map = OrderedMap::<u64, Vec<u8>, _>::new(memory.clone());
            map.insert(7, vec![1; 256]);
            let node = map.store.node(map.store.root);
            match corruption {
                0 => memory.write(node.value, &0_u64.to_le_bytes()),
                1 => memory.write(node.value + 8, &4_u64.to_le_bytes()),
                // Range fits the heap but not this allocation's admitted class.
                _ => {
                    map.insert(9, vec![2; 4096]);
                    memory.write(node.address + 64, &1024_u64.to_le_bytes());
                }
            }
            let writes = memory.writes.get();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    if remove {
                        map.remove_discard(&7)
                    } else {
                        map.insert_discard(7, vec![3; 256])
                    }
                }))
                .is_err()
            );
            assert_eq!(memory.writes.get(), writes);
        }
    }
}

#[test]
fn discard_mutations_reject_stored_fixed_codec_length() {
    for remove in [false, true] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<u64, u64, _>::new(memory.clone());
        map.insert(7, 91);
        memory.write(map.store.root + 64, &7_u64.to_le_bytes());
        let writes = memory.writes.get();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if remove {
                    map.remove_discard(&7)
                } else {
                    map.insert_discard(7, 2)
                }
            }))
            .is_err()
        );
        assert_eq!(memory.writes.get(), writes);
    }
}
