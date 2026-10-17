//! Consumer-level differential, persistence, range and failure tests.
use crate::btree::BTreeMap;
use crate::substrate as ic_stable_structures;
use crate::{Comparator, Config, Error, Lexicographic, Memory};
use ic_stable_structures::VectorMemory;
use std::{cell::Cell, cmp::Ordering, collections::BTreeMap as Model, ops::Bound, rc::Rc};

type Tree = BTreeMap<VectorMemory>;
fn fresh() -> Tree {
    Tree::create(VectorMemory::default(), Config::default(), Lexicographic).unwrap()
}
fn key(n: u64) -> Vec<u8> {
    n.to_be_bytes().to_vec()
}
fn pairs(tree: &Tree) -> Vec<(Vec<u8>, Vec<u8>)> {
    tree.iter()
        .map(|entry| entry.unwrap().into_pair().unwrap())
        .collect()
}
fn assert_model(tree: &Tree, model: &Model<Vec<u8>, Vec<u8>>) {
    tree.validate().unwrap();
    assert_eq!(tree.len(), model.len() as u64);
    assert_eq!(
        pairs(tree),
        model
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        tree.iter()
            .rev()
            .map(|e| e.unwrap().key().to_vec())
            .collect::<Vec<_>>(),
        model.keys().rev().cloned().collect::<Vec<_>>()
    );
}

#[test]
fn split_merge_replace_reopen_and_reuse_pages() {
    let mut tree = fresh();
    let mut model = Model::new();
    for n in 0..2100 {
        let k = key(n);
        let v = vec![u8::try_from(n % 251).unwrap(); 50];
        assert_eq!(tree.insert(&k, &v).unwrap(), model.insert(k, v));
    }
    assert_model(&tree, &model);
    tree = Tree::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    for n in (0..2100).rev() {
        let k = key(n);
        assert_eq!(
            tree.insert(&k, b"x").unwrap(),
            model.insert(k, b"x".to_vec())
        );
    }
    let capacity = tree.memory_bytes();
    // A coprime permutation forces internal deletion, borrowing and merging.
    for n in 0..2100 {
        let k = key((n * 109) % 2100);
        assert_eq!(tree.remove(&k).unwrap(), model.remove(&k));
        if n % 137 == 0 {
            assert_model(&tree, &model);
        }
    }
    assert!(tree.is_empty());
    assert_model(&tree, &model);
    tree = Tree::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    for n in (0..2100).rev() {
        tree.insert(&key(n), &[7; 50]).unwrap();
    }
    tree.validate().unwrap();
    assert!(tree.memory_bytes() <= capacity);
}

#[test]
fn deterministic_mutations_match_standard_map() {
    let mut tree = fresh();
    let mut model = Model::new();
    let mut random = 17u64;
    for step in 0..12000 {
        random = random
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let k = key((random >> 24) % 1200);
        match random % 4 {
            0 => assert_eq!(tree.remove(&k).unwrap(), model.remove(&k)),
            1 => assert_eq!(tree.get(&k).unwrap(), model.get(&k).cloned()),
            _ => {
                let v = vec![
                    u8::try_from(random % 255).unwrap();
                    usize::try_from((random >> 10) % 90).unwrap()
                ];
                assert_eq!(tree.insert(&k, &v).unwrap(), model.insert(k, v));
            }
        }
        if step % 401 == 0 {
            assert_model(&tree, &model);
            tree = Tree::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
        }
    }
    assert_model(&tree, &model);
}

#[test]
fn all_range_bounds_and_interleaved_directions() {
    let mut tree = fresh();
    let mut model = Model::new();
    for n in (0..1000).step_by(3) {
        tree.insert(&key(n), &key(n)).unwrap();
        model.insert(key(n), key(n));
    }
    let mut bounds = vec![Bound::Unbounded];
    for n in [0, 1, 15, 48, 49, 127, 300, 999, 1000] {
        bounds.push(Bound::Included(key(n)));
        bounds.push(Bound::Excluded(key(n)));
    }
    for lower in &bounds {
        for upper in &bounds {
            let within = |k: &&Vec<u8>| {
                let low = match lower {
                    Bound::Unbounded => true,
                    Bound::Included(v) => *k >= v,
                    Bound::Excluded(v) => *k > v,
                };
                let high = match upper {
                    Bound::Unbounded => true,
                    Bound::Included(v) => *k <= v,
                    Bound::Excluded(v) => *k < v,
                };
                low && high
            };
            let expected = model.keys().filter(within).cloned().collect::<Vec<_>>();
            let range = tree.range(
                lower.as_ref().map(Vec::as_slice),
                upper.as_ref().map(Vec::as_slice),
            );
            if let (
                Bound::Included(a) | Bound::Excluded(a),
                Bound::Included(b) | Bound::Excluded(b),
            ) = (lower, upper)
                && a > b
            {
                assert!(matches!(range, Err(Error::InvalidRange)));
                continue;
            }
            let mut range = range.unwrap();
            let (mut front, mut back) = (0, expected.len());
            while front < back {
                assert_eq!(range.next().unwrap().unwrap().key(), expected[front]);
                front += 1;
                if front < back {
                    back -= 1;
                    assert_eq!(range.next_back().unwrap().unwrap().key(), expected[back]);
                }
            }
            assert!(range.next().is_none());
            assert!(range.next_back().is_none());
        }
    }
}

struct Framed;
impl Comparator for Framed {
    const IDENTITY: [u8; 16] = *b"test-framed-key1";
    fn compare(&self, left: &[u8], right: &[u8]) -> Result<Ordering, Error> {
        let decode = |bytes: &[u8]| {
            if bytes.first().map(|n| usize::from(*n) + 1) != Some(bytes.len()) {
                return Err(Error::InvalidKey);
            }
            Ok(bytes[1..].to_vec())
        };
        Ok(decode(left)?.cmp(&decode(right)?))
    }
}

fn framed_key(payload: &[u8]) -> Vec<u8> {
    let mut encoded = vec![u8::try_from(payload.len()).unwrap()];
    encoded.extend_from_slice(payload);
    encoded
}

#[test]
fn semantic_order_survives_mutations_ranges_and_reopening() {
    let config = Config::default();
    let mut tree = BTreeMap::create(VectorMemory::default(), config, Framed).unwrap();
    // Model keys contain decoded components, independent of the stored framing.
    let mut model = Model::new();
    let keys = (0..700)
        .map(|n| format!("{n}{}", "x".repeat(n % 70)).into_bytes())
        .collect::<Vec<_>>();
    for (n, payload) in keys.iter().enumerate() {
        let value = vec![u8::try_from(n % 251).unwrap(); [1, 16, 17, 4073, 9000][n % 5]];
        assert_eq!(
            tree.insert(&framed_key(payload), &value).unwrap(),
            model.insert(payload.clone(), value)
        );
    }
    // Mutate every key in a coprime permutation, including internal separators
    // and overflow keys. Replacements cross inline and overflow boundaries.
    for round in 0..=keys.len() {
        if round % 100 == 0 {
            tree.validate().unwrap();
            tree = BTreeMap::open(tree.into_memory(), config, Framed).unwrap();
            assert_eq!(tree.len(), model.len() as u64);
            let actual = tree
                .iter()
                .map(|entry| {
                    let (encoded, value) = entry.unwrap().into_pair().unwrap();
                    (encoded[1..].to_vec(), value)
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, model.clone().into_iter().collect::<Vec<_>>());

            // Wire order disagrees with semantic order for these endpoints.
            let lower = framed_key(b"100xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
            let upper = framed_key(b"9");
            assert!(lower > upper);
            let mut expected = model
                .iter()
                .filter(|(key, _)| key.as_slice() >= &lower[1..] && key.as_slice() < &upper[1..]);
            let mut range = tree
                .range(Bound::Included(&lower), Bound::Excluded(&upper))
                .unwrap();
            while let Some((key, value)) = expected.next() {
                let entry = range.next().unwrap().unwrap();
                assert_eq!(&entry.key()[1..], key);
                assert_eq!(&entry.value().unwrap(), value);
                if let Some((key, value)) = expected.next_back() {
                    let entry = range.next_back().unwrap().unwrap();
                    assert_eq!(&entry.key()[1..], key);
                    assert_eq!(&entry.value().unwrap(), value);
                }
            }
            assert!(range.next().is_none());
            assert!(range.next_back().is_none());
        }
        if round == keys.len() {
            break;
        }
        let payload = &keys[(round * 109) % keys.len()];
        let encoded = framed_key(payload);
        if round % 3 == 0 {
            let value = vec![0x7e; if round % 2 == 0 { 0 } else { 8192 }];
            assert_eq!(
                tree.insert(&encoded, &value).unwrap(),
                model.insert(payload.clone(), value)
            );
        } else {
            assert_eq!(tree.remove(&encoded).unwrap(), model.remove(payload));
        }
    }
    for (payload, value) in model {
        assert_eq!(tree.remove(&framed_key(&payload)).unwrap(), Some(value));
    }
    tree.validate().unwrap();
    let tree = BTreeMap::open(tree.into_memory(), config, Framed).unwrap();
    assert!(tree.is_empty());
    assert!(tree.iter().next_back().is_none());
}
#[test]
fn semantic_key_order_differs_from_wire_order() {
    let mut tree = BTreeMap::create(VectorMemory::default(), Config::default(), Framed).unwrap();
    for k in [b"\x01z".as_slice(), b"\x02aa", b"\x01b"] {
        tree.insert(k, b"present").unwrap();
    }
    assert_eq!(
        tree.iter()
            .map(|e| e.unwrap().key().to_vec())
            .collect::<Vec<_>>(),
        vec![b"\x02aa".to_vec(), b"\x01b".to_vec(), b"\x01z".to_vec()]
    );
    assert_eq!(tree.insert(b"\x02x", b"bad"), Err(Error::InvalidKey));
    assert_eq!(tree.len(), 3);
    tree.validate().unwrap();
    let memory = tree.into_memory();
    assert!(matches!(
        BTreeMap::open(memory.clone(), Config::default(), Lexicographic),
        Err(Error::Configuration)
    ));
    BTreeMap::open(memory, Config::default(), Framed)
        .unwrap()
        .validate()
        .unwrap();
}

#[derive(Clone, Default)]
struct ObservedMemory {
    memory: VectorMemory,
    reads: Rc<Cell<u64>>,
    writes: Rc<Cell<u64>>,
    read_calls: Rc<Cell<u64>>,
    write_calls: Rc<Cell<u64>>,
    deny_growth: Rc<Cell<bool>>,
}
impl Memory for ObservedMemory {
    fn size(&self) -> u64 {
        self.memory.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        if self.deny_growth.get() {
            -1
        } else {
            self.memory.grow(pages)
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.read_calls.set(self.read_calls.get() + 1);
        self.reads.set(self.reads.get() + bytes.len() as u64);
        self.memory.read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.write_calls.set(self.write_calls.get() + 1);
        self.writes.set(self.writes.get() + bytes.len() as u64);
        self.memory.write(offset, bytes);
    }
}

#[test]
#[ignore = "explicit native stable-memory traffic qualification"]
fn workload_memory_traffic() {
    println!(
        "engine,workload,operation,read_bytes,write_bytes,reserved_bytes,read_calls,write_calls"
    );
    for (name, count, size) in [
        ("index", 2048, 1),
        ("journal", 64, 65536),
        ("row", 4, 4_194_304),
    ] {
        let memory = ObservedMemory::default();
        let reads = memory.reads.clone();
        let writes = memory.writes.clone();
        let read_calls = memory.read_calls.clone();
        let write_calls = memory.write_calls.clone();
        let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
        let value = vec![7; size];
        for n in 0..count {
            tree.insert(&key(n), &value).unwrap();
        }
        for operation in ["get", "replace", "scan_keys", "reverse_keys", "remove"] {
            reads.set(0);
            writes.set(0);
            read_calls.set(0);
            write_calls.set(0);
            match operation {
                "get" => assert_eq!(tree.get(&key(count / 2)).unwrap(), Some(value.clone())),
                "replace" => assert_eq!(
                    tree.insert(&key(count / 2), &value).unwrap(),
                    Some(value.clone())
                ),
                "scan_keys" => {
                    for entry in tree.iter().take(16) {
                        assert_eq!(entry.unwrap().value_len(), size as u64);
                    }
                }
                "reverse_keys" => {
                    for entry in tree.iter().rev().take(16) {
                        assert_eq!(entry.unwrap().value_len(), size as u64);
                    }
                }
                "remove" => assert_eq!(tree.remove(&key(count / 2)).unwrap(), Some(value.clone())),
                _ => unreachable!(),
            }
            println!(
                "ic-storage,{name},{operation},{},{},{},{},{}",
                reads.get(),
                writes.get(),
                tree.memory_bytes(),
                read_calls.get(),
                write_calls.get()
            );
        }
        tree.validate().unwrap();
        let memory = ObservedMemory::default();
        let reads = memory.reads.clone();
        let writes = memory.writes.clone();
        let read_calls = memory.read_calls.clone();
        let write_calls = memory.write_calls.clone();
        let backing = memory.memory.clone();
        let mut upstream = ic_stable_structures::BTreeMap::<Vec<u8>, Vec<u8>, _>::new(memory);
        for n in 0..count {
            upstream.insert(key(n), value.clone());
        }
        for operation in ["get", "replace", "scan_keys", "reverse_keys", "remove"] {
            reads.set(0);
            writes.set(0);
            read_calls.set(0);
            write_calls.set(0);
            match operation {
                "get" => assert_eq!(upstream.get(&key(count / 2)), Some(value.clone())),
                "replace" => assert_eq!(
                    upstream.insert(key(count / 2), value.clone()),
                    Some(value.clone())
                ),
                "scan_keys" => {
                    for entry in upstream.iter().take(16) {
                        assert_eq!(entry.key().len(), 8);
                    }
                }
                "reverse_keys" => {
                    for entry in upstream.iter().rev().take(16) {
                        assert_eq!(entry.key().len(), 8);
                    }
                }
                "remove" => assert_eq!(upstream.remove(&key(count / 2)), Some(value.clone())),
                _ => unreachable!(),
            }
            println!(
                "upstream,{name},{operation},{},{},{},{},{}",
                reads.get(),
                writes.get(),
                backing.size() * 65536,
                read_calls.get(),
                write_calls.get()
            );
        }
    }
}

#[test]
fn replacement_reads_only_one_search_path_and_payload() {
    let memory = ObservedMemory::default();
    let reads = memory.reads.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    for n in 0..1024 {
        tree.insert(&key(n), &[0]).unwrap();
    }
    for n in [0, 15, 511, 1023] {
        reads.set(0);
        assert_eq!(tree.get(&key(n)).unwrap(), Some(vec![0]));
        let lookup_reads = reads.get();
        reads.set(0);
        assert_eq!(tree.insert(&key(n), &[1]).unwrap(), Some(vec![0]));
        assert_eq!(
            reads.get(),
            lookup_reads,
            "replacement must reuse the located node"
        );
    }
    let value = vec![7; 65536];
    tree.insert(&key(0), &value).unwrap();
    reads.set(0);
    assert_eq!(tree.get(&key(0)).unwrap(), Some(value.clone()));
    let lookup_reads = reads.get();
    reads.set(0);
    assert_eq!(tree.insert(&key(0), &value).unwrap(), Some(value));
    assert_eq!(reads.get(), lookup_reads, "old payload must be read once");
    tree.validate().unwrap();
}
#[test]
fn icydb_sized_rows_large_keys_and_lazy_values() {
    let memory = ObservedMemory::default();
    let reads = memory.reads.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    let row = vec![0x5a; 4 * 1024 * 1024];
    let large_key = vec![0x13; 128 * 1024];
    tree.insert(&large_key, &row).unwrap();
    tree.insert(b"marker", &[0]).unwrap();
    tree.insert(b"", b"").unwrap();
    reads.set(0);
    let keys = tree
        .iter()
        .map(|e| e.unwrap().key().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec![vec![], large_key.clone(), b"marker".to_vec()]);
    assert!(
        reads.get() < 256 * 1024,
        "key scan must not read the 4 MiB row"
    );
    assert_eq!(tree.get(&large_key).unwrap().unwrap(), row);
    assert_eq!(
        tree.insert(b"oversize", &vec![0; 4 * 1024 * 1024 + 1]),
        Err(Error::TooLarge)
    );
    tree.validate().unwrap();
    tree.remove(&large_key).unwrap();
    tree.validate().unwrap();
}

#[test]
fn capacity_failure_preserves_all_existing_bytes() {
    let memory = ObservedMemory::default();
    let deny = memory.deny_growth.clone();
    let backing = memory.memory.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    tree.insert(b"kept", b"original").unwrap();
    let before = backing.borrow().clone();
    deny.set(true);
    assert_eq!(
        tree.insert(b"kept", &vec![9; 128 * 1024]),
        Err(Error::Capacity)
    );
    assert_eq!(*backing.borrow(), before);
    assert_eq!(tree.get(b"kept").unwrap(), Some(b"original".to_vec()));
    tree.validate().unwrap();
    deny.set(false);
    tree.insert(b"kept", &vec![9; 128 * 1024]).unwrap();
    tree.validate().unwrap();
}

#[test]
fn reject_foreign_corrupt_and_incompatible_memory() {
    let mut tree = fresh();
    tree.insert(b"key", b"value").unwrap();
    let memory = tree.into_memory();
    assert!(matches!(
        Tree::create(memory.clone(), Config::default(), Lexicographic),
        Err(Error::NotEmpty)
    ));
    let limits = Config {
        max_key_bytes: 10,
        ..Config::default()
    };
    assert!(matches!(
        Tree::open(memory.clone(), limits, Lexicographic),
        Err(Error::Configuration)
    ));
    let mut bytes = memory.borrow().clone();
    memory.write(4096 + 100, &[0xff]);
    assert!(matches!(
        Tree::open(memory.clone(), Config::default(), Lexicographic),
        Err(Error::Corrupt)
    ));
    memory.write(0, &bytes);
    bytes[0] ^= 1;
    memory.write(0, &bytes);
    assert!(matches!(
        Tree::open(memory, Config::default(), Lexicographic),
        Err(Error::Format)
    ));
}

#[test]
fn compact_index_markers_do_not_allocate_a_page_per_entry() {
    let mut tree = fresh();
    for n in 0..10000 {
        tree.insert(&key(n), &[0]).unwrap();
    }
    tree.validate().unwrap();
    assert!(tree.memory_bytes() < 4 * 1024 * 1024);
}

#[test]
fn staged_split_is_discarded_when_growth_is_refused() {
    let memory = ObservedMemory::default();
    let backing = memory.memory.clone();
    let deny = memory.deny_growth.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    for n in 0..31 {
        tree.insert(&key(n), &[0]).unwrap();
    }
    let before = backing.borrow().clone();
    deny.set(true);
    assert_eq!(
        tree.insert(&key(32), &vec![3; 128 * 1024]),
        Err(Error::Capacity)
    );
    assert_eq!(*backing.borrow(), before);
    assert_eq!(tree.len(), 31);
    tree.validate().unwrap();
}

#[test]
fn corrupt_value_is_lazy_and_failed_mutation_preserves_bytes() {
    let mut tree = fresh();
    let value = vec![0xab; 10000];
    tree.insert(b"key", &value).unwrap();
    let memory = tree.into_memory();
    let payload = memory
        .borrow()
        .windows(64)
        .position(|bytes| bytes == [0xab; 64])
        .unwrap();
    memory.write(payload as u64, &[0x7f]);
    let before = memory.borrow().clone();
    let mut tree = Tree::open(memory.clone(), Config::default(), Lexicographic).unwrap();
    let entry = tree.iter().next().unwrap().unwrap();
    assert_eq!(entry.key(), b"key");
    assert_eq!(entry.value_len(), 10000);
    assert!(tree.contains_key(b"key").unwrap());
    assert!(!tree.contains_key(b"missing").unwrap());
    assert_eq!(entry.value(), Err(Error::Corrupt));
    assert_eq!(tree.insert(b"key", b"replacement"), Err(Error::Corrupt));
    assert_eq!(*memory.borrow(), before);
    assert_eq!(tree.remove(b"key"), Err(Error::Corrupt));
    assert_eq!(*memory.borrow(), before);
    assert_eq!(tree.validate(), Err(Error::Corrupt));
}

#[test]
fn iterator_errors_once_and_creation_failure_is_non_destructive() {
    let memory = ObservedMemory::default();
    memory.deny_growth.set(true);
    assert!(matches!(
        BTreeMap::create(memory.clone(), Config::default(), Lexicographic),
        Err(Error::Capacity)
    ));
    assert_eq!(memory.size(), 0);
    memory.deny_growth.set(false);
    let mut tree = BTreeMap::create(memory.clone(), Config::default(), Lexicographic).unwrap();
    tree.insert(b"key", &[1]).unwrap();
    // Deliberately violate exclusive ownership to inject post-open corruption.
    memory.write(4096, &[0xff]);
    let mut range = tree.iter();
    assert!(matches!(range.next(), Some(Err(Error::Corrupt))));
    assert!(range.next_back().is_none());
    assert!(range.next().is_none());
}

#[test]
fn unbounded_cursors_reject_cycles_and_fuse_in_both_directions() {
    use crate::btree::page::{MAX_HEIGHT, SIZE};

    let memory = ObservedMemory::default();
    let mut tree = BTreeMap::create(memory.clone(), Config::default(), Lexicographic).unwrap();
    tree.insert(b"key", &[1]).unwrap();
    let mut root = tree.node(tree.header.root).unwrap();
    // Authenticated corruption: endpoint descent must still have a height bound.
    root.children = vec![tree.header.root; 2];
    memory.write(tree.header.root * SIZE as u64, &*root.encode().unwrap());
    for reverse in [false, true] {
        memory.read_calls.set(0);
        let mut range = tree.iter();
        let result = if reverse {
            range.next_back()
        } else {
            range.next()
        };
        assert!(matches!(result, Some(Err(Error::Corrupt))));
        assert_eq!(memory.read_calls.get(), MAX_HEIGHT as u64);
        assert!(range.next().is_none());
        assert!(range.next_back().is_none());
        assert_eq!(memory.read_calls.get(), MAX_HEIGHT as u64);
    }
}

#[test]
fn replacements_reuse_payload_capacity_across_inline_boundaries() {
    let memory = ObservedMemory::default();
    let deny = memory.deny_growth.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    let mut previous = vec![1; 128 * 1024];
    tree.insert(b"row", &previous).unwrap();
    let capacity = tree.memory_bytes();
    deny.set(true);
    for (round, size) in [128 * 1024, 120 * 1024, 4072, 4073, 17, 16, 0, 128 * 1024]
        .into_iter()
        .enumerate()
    {
        let value = vec![u8::try_from(round + 2).unwrap(); size];
        assert_eq!(tree.insert(b"row", &value).unwrap(), Some(previous));
        assert_eq!(tree.get(b"row").unwrap(), Some(value.clone()));
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.memory_bytes(), capacity);
        tree.validate().unwrap();
        tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
        previous = value;
    }
}

#[test]
fn replacing_full_root_does_not_allocate_or_split_nodes() {
    let memory = ObservedMemory::default();
    let backing = memory.memory.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    for n in 0..31 {
        tree.insert(&key(n), &[0]).unwrap();
    }
    // The persisted header contains root, allocator and entry-count state.
    let header = backing.borrow()[..4096].to_vec();
    for n in 0..31 {
        assert_eq!(tree.insert(&key(n), &[1]).unwrap(), Some(vec![0]));
        assert_eq!(backing.borrow()[..4096], header);
    }
    tree.validate().unwrap();
}

#[test]
fn insertion_reuses_the_validated_leaf() {
    let memory = ObservedMemory::default();
    let reads = memory.read_calls.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    for n in 0..31 {
        reads.set(0);
        assert_eq!(tree.insert(&key(n), &[7]).unwrap(), None);
        // One existence search and one mutation traversal, with no second
        // leaf read merely to check whether it needs splitting.
        assert_eq!(reads.get(), 2);
    }
    // Split the full root, then check reuse while descending through a branch.
    reads.set(0);
    assert_eq!(tree.insert(&key(31), &[7]).unwrap(), None);
    assert_eq!(reads.get(), 2);
    reads.set(0);
    assert_eq!(tree.insert(&key(32), &[7]).unwrap(), None);
    assert_eq!(reads.get(), 4);
    for n in 33..47 {
        assert_eq!(tree.insert(&key(n), &[7]).unwrap(), None);
    }
    // The right child is full: splitting it must reuse the decoded child too.
    reads.set(0);
    assert_eq!(tree.insert(&key(47), &[7]).unwrap(), None);
    assert_eq!(reads.get(), 4);
    tree.validate().unwrap();
    let tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    for n in 0..48 {
        assert_eq!(tree.get(&key(n)).unwrap(), Some(vec![7]));
    }
}

#[test]
fn replacement_writes_only_changed_metadata_and_payload() {
    for length in [0, 1, 16, 17, 4072, 4073, 65536] {
        let memory = ObservedMemory::default();
        let calls = memory.write_calls.clone();
        let deny = memory.deny_growth.clone();
        let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
        let original = vec![7; length];
        tree.insert(b"row", &original).unwrap();
        deny.set(true);
        let changed = vec![8; length];
        calls.set(0);
        assert_eq!(tree.insert(b"row", &changed).unwrap(), Some(original));
        let expected = if length <= 16 {
            u64::from(length != 0)
        } else {
            length.div_ceil(4072) as u64
        };
        assert_eq!(calls.get(), expected, "changed value length {length}");
        tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
        assert_eq!(tree.get(b"row").unwrap(), Some(changed.clone()));
        assert_eq!(tree.len(), 1);
        tree.validate().unwrap();
        calls.set(0);
        assert_eq!(tree.insert(b"row", &changed).unwrap(), Some(changed));
        assert_eq!(calls.get(), if length <= 16 { 0 } else { expected });
        tree.validate().unwrap();
    }
}

#[test]
fn failed_overflow_extension_preserves_reused_pages() {
    let memory = ObservedMemory::default();
    let backing = memory.memory.clone();
    let deny = memory.deny_growth.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    let original = vec![7; 128 * 1024];
    tree.insert(b"row", &original).unwrap();
    let before = backing.borrow().clone();
    deny.set(true);
    assert_eq!(
        tree.insert(b"row", &vec![9; 1024 * 1024]),
        Err(Error::Capacity)
    );
    assert_eq!(*backing.borrow(), before);
    assert_eq!(tree.get(b"row").unwrap(), Some(original));
    tree.validate().unwrap();
}

#[test]
#[ignore = "explicit bounded upstream index traffic qualification"]
fn bounded_upstream_index_memory_traffic() {
    let memory = ObservedMemory::default();
    let reads = memory.reads.clone();
    let writes = memory.writes.clone();
    let read_calls = memory.read_calls.clone();
    let write_calls = memory.write_calls.clone();
    let backing = memory.memory.clone();
    let mut tree = ic_stable_structures::BTreeMap::<[u8; 8], [u8; 1], _>::new(memory);
    for n in 0u64..2048 {
        tree.insert(n.to_be_bytes(), [7]);
    }
    println!(
        "engine,workload,operation,read_bytes,write_bytes,reserved_bytes,read_calls,write_calls"
    );
    for operation in ["get", "replace", "scan_keys", "reverse_keys", "remove"] {
        reads.set(0);
        writes.set(0);
        read_calls.set(0);
        write_calls.set(0);
        let key = 1024u64.to_be_bytes();
        match operation {
            "get" => assert_eq!(tree.get(&key), Some([7])),
            "replace" => assert_eq!(tree.insert(key, [7]), Some([7])),
            "scan_keys" => {
                for entry in tree.iter().take(16) {
                    assert_eq!(entry.key().len(), 8);
                }
            }
            "reverse_keys" => {
                for entry in tree.iter().rev().take(16) {
                    assert_eq!(entry.key().len(), 8);
                }
            }
            "remove" => assert_eq!(tree.remove(&key), Some([7])),
            _ => unreachable!(),
        }
        println!(
            "upstream-bounded,index,{operation},{},{},{},{},{}",
            reads.get(),
            writes.get(),
            backing.size() * 65536,
            read_calls.get(),
            write_calls.get()
        );
    }
}

// Bound metadata from IcyDB e104fe1c9f5921336f0e5b292acbda99f8311476.
// These bytes model lengths and order only; they are not IcyDB wire codecs.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ProfileBytes<const MAX: u32, const FIXED: bool>(Vec<u8>);
impl<const MAX: u32, const FIXED: bool> ic_stable_structures::Storable
    for ProfileBytes<MAX, FIXED>
{
    const BOUND: ic_stable_structures::storable::Bound = if MAX == 0 {
        ic_stable_structures::storable::Bound::Unbounded
    } else {
        ic_stable_structures::storable::Bound::Bounded {
            max_size: MAX,
            is_fixed_size: FIXED,
        }
    };
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        std::borrow::Cow::Borrowed(&self.0)
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0
    }
    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        Self(bytes.into_owned())
    }
}
fn reset_traffic(memory: &ObservedMemory) {
    memory.reads.set(0);
    memory.writes.set(0);
    memory.read_calls.set(0);
    memory.write_calls.set(0);
}
fn report_traffic(engine: &str, profile: &str, operation: &str, memory: &ObservedMemory) {
    println!(
        "{engine},{profile},{operation},{},{},{},{},{}",
        memory.reads.get(),
        memory.writes.get(),
        memory.size() * 65536,
        memory.read_calls.get(),
        memory.write_calls.get()
    );
}
// Keep the shared operation sequence together so both engines receive the same workload.
#[allow(clippy::too_many_lines)]
fn compare_bound_profile<const KMAX: u32, const VMAX: u32, const KFIXED: bool>(
    name: &str,
    count: u64,
    key_len: usize,
    value_len: usize,
) {
    let own_memory = ObservedMemory::default();
    let other_memory = ObservedMemory::default();
    let mut own = BTreeMap::create(own_memory.clone(), Config::default(), Lexicographic).unwrap();
    let mut other = ic_stable_structures::BTreeMap::<
        ProfileBytes<KMAX, KFIXED>,
        ProfileBytes<VMAX, false>,
        _,
    >::new(other_memory.clone());
    let encode_key = |n: u64| {
        let mut bytes = vec![0; key_len];
        bytes[..8].copy_from_slice(&n.to_be_bytes());
        bytes
    };
    let value = vec![7; value_len];
    for n in 0..count {
        let key = encode_key(n);
        assert!(own.insert(&key, &value).unwrap().is_none());
        assert!(
            other
                .insert(ProfileBytes(key), ProfileBytes(value.clone()))
                .is_none()
        );
    }
    // Drop both handles and reopen their own formats before measurement.
    own = BTreeMap::open(own.into_memory(), Config::default(), Lexicographic).unwrap();
    other = ic_stable_structures::BTreeMap::load(other.into_memory());
    for operation in [
        "get",
        "replace",
        "scan_keys",
        "remove",
        "restore",
        "shrink_all",
        "grow_all",
        "delete_even",
        "refill_even",
    ] {
        reset_traffic(&own_memory);
        reset_traffic(&other_memory);
        let key = encode_key(count / 2);
        match operation {
            "get" => {
                assert_eq!(own.get(&key).unwrap(), Some(value.clone()));
                assert_eq!(other.get(&ProfileBytes(key)).unwrap().0, value);
            }
            "replace" => {
                assert_eq!(own.insert(&key, &value).unwrap(), Some(value.clone()));
                assert_eq!(
                    other
                        .insert(ProfileBytes(key), ProfileBytes(value.clone()))
                        .unwrap()
                        .0,
                    value
                );
            }
            "scan_keys" => {
                let a = own
                    .iter()
                    .take(16)
                    .map(|entry| entry.unwrap().key().to_vec())
                    .collect::<Vec<_>>();
                let b = other
                    .iter()
                    .take(16)
                    .map(|entry| entry.key().0.clone())
                    .collect::<Vec<_>>();
                assert_eq!(a, b);
            }
            "remove" => {
                assert_eq!(own.remove(&key).unwrap(), Some(value.clone()));
                assert_eq!(other.remove(&ProfileBytes(key)).unwrap().0, value);
            }
            "restore" => {
                assert!(own.insert(&key, &value).unwrap().is_none());
                assert!(
                    other
                        .insert(ProfileBytes(key), ProfileBytes(value.clone()))
                        .is_none()
                );
            }
            "shrink_all" | "grow_all" => {
                let small = vec![7; value_len.min(16)];
                let (old, new) = if operation == "shrink_all" {
                    (&value, &small)
                } else {
                    (&small, &value)
                };
                for n in 0..count {
                    let key = encode_key((n * 109) % count);
                    assert_eq!(own.insert(&key, new).unwrap().as_ref(), Some(old));
                    assert_eq!(
                        &other
                            .insert(ProfileBytes(key), ProfileBytes(new.clone()))
                            .unwrap()
                            .0,
                        old
                    );
                }
            }
            "delete_even" => {
                for n in (0..count).step_by(2) {
                    let key = encode_key(n);
                    assert_eq!(own.remove(&key).unwrap(), Some(value.clone()));
                    assert_eq!(other.remove(&ProfileBytes(key)).unwrap().0, value);
                }
            }
            "refill_even" => {
                for n in (0..count).rev().filter(|n| n % 2 == 0) {
                    let key = encode_key(n);
                    assert!(own.insert(&key, &value).unwrap().is_none());
                    assert!(
                        other
                            .insert(ProfileBytes(key), ProfileBytes(value.clone()))
                            .is_none()
                    );
                }
            }
            _ => unreachable!(),
        }
        report_traffic("ic-storage", name, operation, &own_memory);
        report_traffic("upstream", name, operation, &other_memory);
        // Audit outside measurement, then reopen between phases to exercise
        // persisted free lists rather than relying on retained heap state.
        own.validate().unwrap();
        assert_eq!(own.len(), other.len());
        own = BTreeMap::open(own.into_memory(), Config::default(), Lexicographic).unwrap();
        other = ic_stable_structures::BTreeMap::load(other.into_memory());
    }
    own.validate().unwrap();
    assert_eq!(own.len(), other.len());
    let own = BTreeMap::open(own.into_memory(), Config::default(), Lexicographic).unwrap();
    let other = ic_stable_structures::BTreeMap::<
        ProfileBytes<KMAX, KFIXED>,
        ProfileBytes<VMAX, false>,
        _,
    >::load(other.into_memory());
    let mut seen = 0;
    for (a, b) in own.iter().zip(other.iter()) {
        let a = a.unwrap();
        assert_eq!(a.key(), b.key().0);
        assert_eq!(a.value().unwrap(), b.value().0);
        seen += 1;
    }
    assert_eq!(own.len(), other.len());
    assert_eq!(seen, own.len());
}
#[test]
fn icydb_bound_profiles_reopen_and_mutate() {
    compare_bound_profile::<16668, 1, false>("index-smoke", 64, 64, 1);
    compare_bound_profile::<16668, 1, false>("index-max", 16, 16668, 1);
    compare_bound_profile::<12, 65536, true>("journal-smoke", 16, 12, 65536);
    compare_bound_profile::<262, 0, false>("row-smoke", 16, 262, 4096);
}
#[test]
#[ignore = "explicit IcyDB-bound metadata comparison"]
fn icydb_bound_profiles_memory_traffic() {
    println!(
        "engine,profile,operation,read_bytes,write_bytes,reserved_bytes,read_calls,write_calls"
    );
    compare_bound_profile::<16668, 1, false>("index-24", 2048, 24, 1);
    compare_bound_profile::<16668, 1, false>("index-64", 2048, 64, 1);
    compare_bound_profile::<16668, 1, false>("index-1024", 512, 1024, 1);
    compare_bound_profile::<12, 65536, true>("journal-4k", 64, 12, 4096);
    compare_bound_profile::<12, 65536, true>("journal-64k", 64, 12, 65536);
    compare_bound_profile::<262, 0, false>("row-4k", 64, 25, 4096);
    compare_bound_profile::<262, 0, false>("row-4m", 4, 25, 4 * 1024 * 1024);
}

#[test]
fn removal_avoids_duplicate_overflow_page_reads() {
    let memory = ObservedMemory::default();
    let reads = memory.reads.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    let value = vec![9; 65536];
    tree.insert(b"row", &value).unwrap();
    reads.set(0);
    assert_eq!(tree.get(b"row").unwrap(), Some(value.clone()));
    let lookup_reads = reads.get();
    reads.set(0);
    assert_eq!(tree.remove(b"row").unwrap(), Some(value));
    assert!(
        reads.get() <= lookup_reads + 8192,
        "removal should collect the returned payload while reclaiming its pages in one pass"
    );
    tree.validate().unwrap();
    let tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    assert!(tree.is_empty());
}

#[test]
fn late_corruption_discards_collected_removal_and_staged_pages() {
    let memory = ObservedMemory::default();
    let backing = memory.memory.clone();
    let writes = memory.write_calls.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    let mut value = vec![0xab; 10000];
    value[9000..].fill(0xcd);
    tree.insert(b"row", &value).unwrap();
    // Mark the tail of a multi-page payload, after valid pages were traversed.
    let tail = backing
        .borrow()
        .windows(64)
        .position(|bytes| bytes == [0xcd; 64])
        .unwrap();
    backing.write(tail as u64, &[0x7f]);
    let before = backing.borrow().clone();
    writes.set(0);
    assert_eq!(tree.remove(b"row"), Err(Error::Corrupt));
    assert_eq!(writes.get(), 0);
    assert_eq!(*backing.borrow(), before);
    assert_eq!(tree.len(), 1);
    assert!(tree.contains_key(b"row").unwrap());
    assert_eq!(tree.remove(b"missing").unwrap(), None);
    assert_eq!(writes.get(), 0);

    // Repair only the injected corruption; the failed transaction changed nothing.
    backing.write(tail as u64, &[0xcd]);
    assert_eq!(tree.remove(b"row").unwrap(), Some(value));
    tree.validate().unwrap();
    let tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    assert!(tree.is_empty());
}

#[test]
fn shrinking_replacement_reads_each_overflow_page_once() {
    for length in [1, 4073] {
        let memory = ObservedMemory::default();
        let reads = memory.reads.clone();
        let deny_growth = memory.deny_growth.clone();
        let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
        let original = vec![9; 65536];
        tree.insert(b"row", &original).unwrap();
        reads.set(0);
        assert_eq!(tree.get(b"row").unwrap(), Some(original.clone()));
        let lookup_reads = reads.get();
        reads.set(0);
        deny_growth.set(true);
        let replacement = vec![3; length];
        assert_eq!(tree.insert(b"row", &replacement).unwrap(), Some(original));
        assert!(
            reads.get() <= lookup_reads,
            "shrinking should not reread validated pages to free them"
        );
        tree.validate().unwrap();
        let tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
        assert_eq!(tree.get(b"row").unwrap(), Some(replacement));
    }
}

#[test]
fn overflow_key_corruption_preserves_published_bytes() {
    // Exercise a short overflow key, an exact payload page and a chained key.
    for len in [33, 4072, 4073] {
        let key = vec![0xa7; len];
        let mut tree = fresh();
        tree.insert(&key, b"value").unwrap();
        let memory = tree.into_memory();
        let payload = memory
            .borrow()
            .windows(33)
            .position(|bytes| bytes == [0xa7; 33])
            .unwrap();
        let start = payload / 4096 * 4096;
        let original = memory.borrow()[start..start + 4096].to_vec();
        for fault in 0..3 {
            let mut page = original.clone();
            match fault {
                0 => page[16] ^= 1, // Payload damage with the original checksum.
                1 => page[0] = 1,   // Authenticated wrong page kind.
                _ => {
                    // An authenticated nonterminal chain must not be accepted
                    // merely because enough key bytes have been collected.
                    let self_id = u64::try_from(start / 4096).unwrap();
                    page[8..16].copy_from_slice(&self_id.to_le_bytes());
                }
            }
            if fault != 0 {
                let checksum = page[..4088]
                    .iter()
                    .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
                    });
                page[4088..].copy_from_slice(&checksum.to_le_bytes());
            }
            memory.write(u64::try_from(start).unwrap(), &page);
            let before = memory.borrow().clone();
            let mut tree = Tree::open(memory.clone(), Config::default(), Lexicographic).unwrap();
            assert_eq!(tree.get(&key), Err(Error::Corrupt));
            assert_eq!(tree.contains_key(&key), Err(Error::Corrupt));
            assert!(matches!(tree.iter().next(), Some(Err(Error::Corrupt))));
            assert_eq!(tree.insert(&key, b"changed"), Err(Error::Corrupt));
            assert_eq!(tree.remove(&key), Err(Error::Corrupt));
            assert_eq!(tree.validate(), Err(Error::Corrupt));
            assert_eq!(*memory.borrow(), before);
            memory.write(u64::try_from(start).unwrap(), &original);
        }
        let tree = Tree::open(memory, Config::default(), Lexicographic).unwrap();
        assert_eq!(tree.get(&key).unwrap(), Some(b"value".to_vec()));
        tree.validate().unwrap();
    }
}

#[test]
fn validation_reads_each_live_or_free_page_once() {
    use crate::btree::page::SIZE;

    for length in [0, 32, 33, 4072, 4073, 16668] {
        let memory = ObservedMemory::default();
        let mut tree = BTreeMap::create(memory.clone(), Config::default(), Lexicographic).unwrap();
        tree.insert(&vec![0xa7; length], &vec![0xb7; 9000]).unwrap();
        // Retain a free chain as well as live key/value chains.
        tree.insert(b"temporary", &vec![0xc7; 20000]).unwrap();
        tree.remove(b"temporary").unwrap();
        assert_ne!(tree.header.free, 0);
        let pages = tree.header.next - 1;
        memory.read_calls.set(0);
        memory.reads.set(0);
        memory.write_calls.set(0);
        tree.validate().unwrap();
        assert_eq!(memory.read_calls.get(), pages, "key length {length}");
        assert_eq!(memory.reads.get(), pages * SIZE as u64);
        assert_eq!(memory.write_calls.get(), 0);
    }
}

#[test]
fn validation_rejects_shared_key_value_pages() {
    use crate::btree::page::SIZE;

    let memory = ObservedMemory::default();
    let mut tree = BTreeMap::create(memory.clone(), Config::default(), Lexicographic).unwrap();
    let key = vec![0xa7; 4073];
    tree.insert(&key, &vec![0xb7; key.len()]).unwrap();
    let mut root = tree.node(tree.header.root).unwrap();
    root.entries[0].value = root.entries[0].key;
    memory.write(tree.header.root * SIZE as u64, &*root.encode().unwrap());
    // Both descriptors decode and read successfully; only the ownership audit
    // can reject the shared pages (and the now-unreachable original value).
    assert_eq!(tree.get(&key).unwrap(), Some(key.clone()));
    let before = memory.memory.borrow().clone();
    memory.write_calls.set(0);
    assert_eq!(tree.validate(), Err(Error::Corrupt));
    assert_eq!(memory.write_calls.get(), 0);
    assert_eq!(*memory.memory.borrow(), before);
}

#[test]
fn deletion_searches_once_and_discards_balancing_on_a_miss() {
    let memory = ObservedMemory::default();
    let reads = memory.read_calls.clone();
    let writes = memory.write_calls.clone();
    let backing = memory.memory.clone();
    let mut tree = BTreeMap::create(memory, Config::default(), Lexicographic).unwrap();
    tree.insert(&key(0), &[7]).unwrap();
    reads.set(0);
    assert_eq!(tree.remove(&key(0)).unwrap(), Some(vec![7]));
    assert_eq!(reads.get(), 1, "deletion must not preflight membership");
    for n in 0..32 {
        tree.insert(&key(n), &[7]).unwrap();
    }
    tree.remove(&key(31)).unwrap();
    // Two minimum-size children: descent for an absent key stages a merge.
    let before = backing.borrow().clone();
    reads.set(0);
    assert!(!tree.contains_key(&key(999)).unwrap());
    let preflight_reads = reads.get();
    reads.set(0);
    writes.set(0);
    assert_eq!(tree.remove(&key(999)).unwrap(), None);
    eprintln!(
        "minimum-child miss: membership={preflight_reads} reads, transactional={} reads",
        reads.get()
    );
    assert_eq!(writes.get(), 0);
    assert_eq!(*backing.borrow(), before);
    assert_eq!(tree.len(), 31);
    tree.validate().unwrap();
    let tree = BTreeMap::open(tree.into_memory(), Config::default(), Lexicographic).unwrap();
    for n in 0..31 {
        assert_eq!(tree.get(&key(n)).unwrap(), Some(vec![7]));
    }
}

struct ValidationCount(Rc<Cell<u64>>);
impl Comparator for ValidationCount {
    const IDENTITY: [u8; 16] = *b"validation-count";
    fn compare(&self, left: &[u8], right: &[u8]) -> Result<Ordering, Error> {
        // The operation validates a borrowed key by passing the same slice twice;
        // search comparisons use separately decoded stored keys.
        if std::ptr::eq(left, right) {
            self.0.set(self.0.get() + 1);
        }
        if left.starts_with(b"!") || right.starts_with(b"!") {
            return Err(Error::InvalidKey);
        }
        Ok(left.cmp(right))
    }
}

#[test]
fn operations_validate_keys_once_before_value_limits() {
    let calls = Rc::new(Cell::new(0));
    let backing = VectorMemory::default();
    let mut tree = BTreeMap::create(
        backing.clone(),
        Config {
            max_key_bytes: 64,
            max_value_bytes: 1,
        },
        ValidationCount(calls.clone()),
    )
    .unwrap();
    for previous in [None, Some(vec![7])] {
        calls.set(0);
        assert_eq!(tree.insert(b"key", &[7]).unwrap(), previous);
        assert_eq!(calls.get(), 1);
    }
    calls.set(0);
    assert_eq!(tree.get(b"key").unwrap(), Some(vec![7]));
    assert_eq!(calls.get(), 1);
    calls.set(0);
    assert!(tree.contains_key(b"key").unwrap());
    assert_eq!(calls.get(), 1);
    let before = backing.borrow().clone();
    calls.set(0);
    assert_eq!(tree.insert(b"!invalid", &[0, 1]), Err(Error::InvalidKey));
    assert_eq!(calls.get(), 1, "key rejection must precede the value limit");
    assert_eq!(tree.insert(b"valid", &[0, 1]), Err(Error::TooLarge));
    calls.set(0);
    assert_eq!(tree.insert(&[0; 65], &[]), Err(Error::TooLarge));
    assert_eq!(
        calls.get(),
        0,
        "key size is checked before invoking its codec"
    );
    assert_eq!(*backing.borrow(), before);
    tree.validate().unwrap();
}
