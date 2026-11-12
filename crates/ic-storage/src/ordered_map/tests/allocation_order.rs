use super::*;

#[test]
fn fresh_node_keeps_full_overflow_key_adjacent_ahead_of_large_value() {
    for value_size in [25, 128, 512, 2048, 8192] {
        let memory = TestMemory::default();
        let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
        let key = vec![7; 31];
        let value = vec![3; value_size];
        map.insert(key.clone(), value.clone());
        let node = map.store.root_node().unwrap();
        assert_eq!(node.key, node.address + 128);
        assert_eq!(node.value, node.key + 64);
        assert_eq!(map.get(&key), Some(value.clone()));
        let reopened = OrderedMap::<Vec<u8>, Vec<u8>, _>::load(memory).unwrap();
        assert_eq!(reopened.get(&key), Some(value));
    }
}

#[test]
fn node_first_allocation_preserves_reuse_rotations_and_full_key_comparison() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
    let key = |n: u16| {
        let mut key = vec![7; 31];
        key[29..].copy_from_slice(&n.to_be_bytes());
        key
    };
    for cycle in 0..4 {
        for n in 0..256 {
            assert_eq!(map.insert(key(n), vec![cycle; 512]), None);
        }
        for n in 0..256 {
            assert_eq!(map.get(&key(n)), Some(vec![cycle; 512]));
        }
        for n in (0..256).rev() {
            assert_eq!(map.remove(&key(n)), Some(vec![cycle; 512]));
        }
        assert!(map.is_empty());
        map = OrderedMap::load(memory.clone()).unwrap();
    }
}

#[test]
fn existing_key_value_node_allocation_order_remains_readable_and_mutable() {
    // Produce one valid V1 image, then relocate its three blocks to the previous
    // allocation order. Absolute pointers, not allocation sequence, own the ABI.
    let memory = TestMemory::default();
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
    let key = vec![9; 31];
    let value = vec![4; 512];
    map.insert(key.clone(), value.clone());
    let node = map.store.root_node().unwrap();
    let read = |at, size| {
        let mut bytes = vec![0; size];
        memory.read(at, &mut bytes);
        bytes
    };
    let mut node_bytes = read(node.address, 128);
    let key_bytes = read(node.key, 64);
    let value_bytes = read(node.value, 1024);
    let old_key = node.address;
    let old_value = old_key + 64;
    let old_node = old_value + 1024;
    node_bytes[40..48].copy_from_slice(&old_key.to_le_bytes());
    node_bytes[56..64].copy_from_slice(&old_value.to_le_bytes());
    memory.write(old_key, &key_bytes);
    memory.write(old_value, &value_bytes);
    memory.write(old_node, &node_bytes);
    memory.write(16, &old_node.to_le_bytes());
    drop(map);
    let mut reopened = OrderedMap::<Vec<u8>, Vec<u8>, _>::load(memory.clone()).unwrap();
    assert_eq!(reopened.get(&key), Some(value));
    assert_eq!(reopened.insert(vec![8; 31], vec![2; 1024]), None);
    assert_eq!(reopened.remove(&key), Some(vec![4; 512]));
    assert_eq!(
        OrderedMap::<Vec<u8>, Vec<u8>, _>::load(memory)
            .unwrap()
            .get(&vec![8; 31]),
        Some(vec![2; 1024])
    );
}

#[test]
fn same_class_free_cycle_cannot_reallocate_a_reserved_node() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
    map.insert(vec![1], Vec::new());
    map.insert(vec![2], Vec::new());
    let first = map.store.root_node().unwrap();
    let second = first.right.address;
    assert_eq!(map.remove(&vec![1]), Some(Vec::new()));
    assert_eq!(map.remove(&vec![2]), Some(Vec::new()));
    // Both nodes are class7 blocks. Their free chain is second -> first -> 0;
    // corrupt only its tail to form second -> first -> second.
    memory.write(first.address + 16, &second.to_le_bytes());
    assert!(catch_unwind(AssertUnwindSafe(|| map.insert(vec![7; 62], vec![9; 62]))).is_err());
    // The allocator may have partially changed its own metadata before trapping,
    // but the failed insertion must not publish an aliased node/value record.
    let mut publication = [0; 16];
    memory.read(16, &mut publication);
    assert_eq!(publication, [0; 16]);
    assert!(map.is_empty());
}

#[test]
fn node_and_both_payloads_reuse_the_same_class_without_aliasing() {
    let memory = TestMemory::default();
    let mut map = OrderedMap::<Vec<u8>, Vec<u8>, _>::new(memory.clone());
    memory.growth_limit.set(Some(memory.size()));
    let key = |n: u8| {
        let mut bytes = vec![7; 62];
        bytes[61] = n;
        bytes
    };
    for cycle in 0..4 {
        for n in 0..64 {
            assert_eq!(map.insert(key(n), vec![cycle; 62]), None);
        }
        let root = map.store.root_node().unwrap();
        assert_ne!(root.address, root.key);
        assert_ne!(root.address, root.value);
        assert_ne!(root.key, root.value);
        map = OrderedMap::load(memory.clone()).unwrap();
        for n in 0..64 {
            assert_eq!(map.get(&key(n)), Some(vec![cycle; 62]));
        }
        for n in (0..64).rev() {
            assert_eq!(map.remove(&key(n)), Some(vec![cycle; 62]));
        }
        assert!(map.is_empty());
        map = OrderedMap::load(memory.clone()).unwrap();
    }
}
