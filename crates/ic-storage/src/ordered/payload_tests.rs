extern crate std;
use super::*;
use alloc::{vec, vec::Vec};
use std::collections::BTreeMap;

#[test]
fn compact_full_rows_and_indexes_use_less_than_half_the_former_reserved_bytes() {
    let mut map = Map::<512, 4096>::new(128);
    for id in 0..64u64 {
        let mut primary = vec![1];
        primary.extend_from_slice(&id.to_be_bytes());
        let mut index = vec![2, (id % 8) as u8];
        index.extend_from_slice(&id.to_be_bytes());
        map.with(|mut v| {
            v.apply(&[
                Mutation::Put(&primary, &[7; 4096]),
                Mutation::Put(&index, &primary),
            ])
        })
        .unwrap()
        .unwrap();
    }
    let before = map.checkpoint(7).unwrap();
    let usage = map.allocation_usage().unwrap();
    let image = map.release().unwrap();
    assert_eq!(size_of::<Node<512, 4096>>(), 56);
    assert_eq!(image.nodes.len() * 56 + image.payload.len(), 273_408);
    assert_eq!(usage.initialized_bytes, 273_408);
    assert_eq!(
        usage.allocated_bytes,
        image.nodes.capacity() * 56 + image.payload.capacity()
    );
    let mut map = Map::admit(image, 128).unwrap_or_else(|_| panic!("compact admission"));
    assert_eq!(map.checkpoint(7).unwrap(), before);
    // Portable bytes remain independent of the native spans and free-list shape.
    let fresh = Map::<512, 4096>::from_checkpoint_in(&before, 7, 128, Global).unwrap();
    assert_eq!(fresh.checkpoint(7).unwrap(), before);
    map.with(|mut v| {
        let keys: Vec<_> = v.prefix(&[]).map(|(k, _)| k.as_slice().to_vec()).collect();
        for key in keys {
            v.remove(&key);
        }
    })
    .unwrap();
    let image = map.release().unwrap();
    let size = image.payload.len();
    let mut map = Map::admit(image, 128).unwrap_or_else(|_| panic!("free admission"));
    // Splitting a retired large block supplies smaller values without growing.
    for id in 0..128u64 {
        map.with(|mut v| v.insert(&id.to_be_bytes(), &[3; 64]))
            .unwrap()
            .unwrap();
    }
    assert_eq!(map.image.payload.len(), size);
    tree::validate(&map.image, 128).unwrap();
}

#[test]
fn variable_payload_batches_match_oracle_through_replacement_split_and_readmission() {
    let mut map = Map::<8, 512>::new(32);
    let mut oracle = BTreeMap::new();
    let mut seed = 0x1234_9876u64;
    for step in 0..2000 {
        let mut inputs = Vec::new();
        for _ in 0..6 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let key = (seed % 32).to_be_bytes();
            let length = [0, 1, 7, 8, 9, 16, 31, 32, 33, 128, 255, 512][(seed >> 8) as usize % 12];
            inputs.push((key, (seed & 3 != 0).then(|| vec![step as u8; length])));
        }
        let mutations: Vec<_> = inputs
            .iter()
            .map(|(k, v)| match v {
                Some(v) => Mutation::Put(k, v),
                None => Mutation::Remove(k),
            })
            .collect();
        map.with(|mut v| v.apply(&mutations)).unwrap().unwrap();
        for (k, v) in inputs {
            if let Some(v) = v {
                oracle.insert(k.to_vec(), v);
            } else {
                oracle.remove(k.as_slice());
            }
        }
        if step % 11 == 0 {
            let image = map.release().unwrap();
            map = Map::admit(image, 32).unwrap_or_else(|_| panic!("step {step}"));
            map.with(|v| {
                let actual: BTreeMap<_, _> = v
                    .prefix(&[])
                    .map(|(k, v)| (k.as_slice().to_vec(), v.as_slice().to_vec()))
                    .collect();
                assert_eq!(actual, oracle);
            })
            .unwrap();
        }
    }
    tree::validate(&map.image, 32).unwrap();
}

#[test]
fn payload_directory_corruption_aliases_and_orphans_refuse_without_writes() {
    let mut map = Map::<8, 64>::new(8);
    map.with(|mut v| {
        v.insert(b"a", &[1; 33]).unwrap();
        v.insert(b"b", &[2; 9]).unwrap();
        v.remove(b"a");
    })
    .unwrap();
    let original = map.release().unwrap();
    for case in 0..arena::HEADER + 7 {
        let mut image = Image {
            nodes: original.nodes.clone(),
            payload: original.payload.clone(),
            root: original.root,
            free: original.free,
            len: original.len,
        };
        let root = image.root as usize - 1;
        if case < arena::HEADER {
            image.payload[case] ^= 1;
        } else {
            match case - arena::HEADER {
                0 => image.nodes[root].value.offset = image.nodes[root].key.offset,
                1 => image.nodes[root].value.offset += 1,
                2 => image.nodes[root].value.offset = 8,
                3 => image.nodes[root].value.length = 0,
                4 => image.payload.push(0),
                5 => {
                    image.payload.pop();
                }
                6 => image.payload[image.nodes[root].value.offset as usize + 15] = 1,
                _ => unreachable!(),
            }
        }
        let nodes = image.nodes.clone();
        let bytes = image.payload.clone();
        let pointer = image.payload.as_ptr();
        let err = match Map::admit(image, 8) {
            Ok(_) => panic!("accepted {case}"),
            Err(err) => err,
        };
        assert_eq!(err.error, Error::NativeImage);
        assert_eq!(err.image.payload.as_ptr(), pointer);
        assert_eq!(err.image.payload, bytes);
        assert_eq!(err.image.nodes, nodes);
    }
}
