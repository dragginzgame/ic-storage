//! Node IO, logical extent accounting, overflow tails and freed-page reuse.
use super::*;
use crate::{VectorMemory, testing::MeteredMemory};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Key(u64);

impl crate::engine::Storable for Key {
    const BOUND: crate::engine::storable::Bound = crate::engine::storable::Bound::Bounded {
        max_size: 8,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.to_le_bytes().to_vec().into()
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }
    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        Self(u64::from_le_bytes(bytes.as_ref().try_into().unwrap()))
    }
}

#[test]
fn empty_values_load_and_extract_without_payload_reads() {
    use super::super::{Node, NodeType};

    let backing = MeteredMemory::<VectorMemory>::default();
    let mut allocator = Allocator::new(backing.clone(), Address::from(0), Bytes::from(128u32));
    let address = allocator.allocate();
    let mut node = Node::new(address, NodeType::Leaf, PageSize(128));
    node.push_entry((Key(1), vec![]));
    node.push_entry((Key(2), vec![7, 8, 9]));
    node.save(&mut allocator);
    let mut node = Node::<Key>::load(address, PageSize(128), allocator.memory());
    backing.reset();
    assert_eq!(node.value(0, allocator.memory()), [] as [u8; 0]);
    assert_eq!(backing.counts().reads, 0);
    assert_eq!(node.value(1, allocator.memory()), [7, 8, 9]);
    assert_eq!(backing.counts().reads, 1);
    assert_eq!(backing.counts().read_bytes, 3);
    // Reload to exercise extracting a still-lazy empty value, not its cached copy.
    node = Node::load(address, PageSize(128), allocator.memory());
    backing.reset();
    assert_eq!(
        node.swap_value(0, vec![4], allocator.memory()),
        [] as [u8; 0]
    );
    assert_eq!(backing.counts().reads, 0);
    node.save(&mut allocator);
    let node = Node::<Key>::load(address, PageSize(128), allocator.memory());
    assert_eq!(node.value(0, allocator.memory()), [4]);
    assert_eq!(node.value(1, allocator.memory()), [7, 8, 9]);
}

#[test]
fn empty_fields_preserve_logical_extent_without_payload_writes() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let mut allocator = Allocator::new(backing.clone(), Address::from(0), Bytes::from(128u32));
    let address = allocator.allocate();
    let mut writer = NodeWriter::new(address, vec![], PageSize(128), &mut allocator);
    backing.reset();
    for offset in [0, 15, 127, 128] {
        writer.write(Address::from(offset), &[]);
        assert_eq!(writer.max_offset, offset);
        assert_eq!(writer.overflows, []);
    }
    assert_eq!(backing.counts().writes, 0);

    // Empty writes beyond the initial page still reserve the logical extent.
    // Skipping them before accounting/allocation would change this behavior.
    writer.write(Address::from(129), &[]);
    assert_eq!(writer.max_offset, 129);
    assert_eq!(writer.overflows.len(), 1);
    backing.reset();
    writer.write(Address::from(15), &[]);
    assert_eq!(writer.max_offset, 129);
    let overflows = writer.finish();
    assert_eq!(overflows.len(), 1);
    assert_eq!(backing.counts().writes, 0);
    assert_eq!(
        crate::engine::read_u64(allocator.memory(), overflows[0] + PAGE_OVERFLOW_NEXT_OFFSET),
        NULL.get(),
    );
}

#[test]
fn overflow_tails_stay_terminated_without_redundant_finish_writes() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let mut allocator = Allocator::new(backing.clone(), Address::from(0), Bytes::from(128u32));
    let address = allocator.allocate();
    let mut overflows = Vec::new();
    // 113 initial payload bytes and 117 per overflow page. Cover exact ends,
    // unchanged chains, partial/full shrinkage, then reusing freed pages.
    for (length, expected_pages) in [
        (0, 0),
        (113, 0),
        (114, 1),
        (230, 1),
        (231, 2),
        (500, 4),
        (500, 4),
        (231, 2),
        (230, 1),
        (114, 1),
        (113, 0),
        (500, 4),
        (0, 0),
        (231, 2),
    ] {
        let old_pages = overflows.len();
        let first = overflows.first().copied().unwrap_or(NULL);
        let mut writer = NodeWriter::new(address, overflows, PageSize(128), &mut allocator);
        writer.write(Address::from(7), &first.get().to_le_bytes());
        let payload = vec![0x5a; length];
        writer.write(Address::from(15), &payload);
        backing.reset();
        overflows = writer.finish();
        let counts = backing.counts();
        assert!(!counts.overflowed);
        assert_eq!(overflows.len(), expected_pages);
        if expected_pages >= old_pages {
            assert_eq!(counts.writes, 0, "unchanged/new tails need no repair");
        } else {
            assert!(counts.writes > 0, "shortened chains need a new terminator");
        }
        let mut link = Address::from(crate::engine::read_u64(
            allocator.memory(),
            address + OVERFLOW_ADDRESS_OFFSET,
        ));
        for page in &overflows {
            assert_eq!(link, *page);
            let mut magic = [0; 3];
            allocator.memory().read(page.get(), &mut magic);
            assert_eq!(&magic, OVERFLOW_MAGIC);
            link = Address::from(crate::engine::read_u64(
                allocator.memory(),
                *page + PAGE_OVERFLOW_NEXT_OFFSET,
            ));
        }
        assert_eq!(link, NULL);
        let reader = NodeReader {
            address,
            overflows: &overflows,
            page_size: PageSize(128),
            memory: allocator.memory(),
        };
        let mut restored = vec![0; length];
        reader.read(15, &mut restored);
        assert_eq!(restored, payload);
    }
}

#[test]
fn reused_overflow_page_clears_stale_link_before_becoming_the_tail() {
    let backing = MeteredMemory::<VectorMemory>::default();
    let mut allocator = Allocator::new(backing.clone(), Address::from(0), Bytes::from(128u32));
    let address = allocator.allocate();
    let freed = allocator.allocate();
    backing.write(freed.get(), &[0xa5; 128]);
    allocator.deallocate(freed);

    let mut writer = NodeWriter::new(address, vec![], PageSize(128), &mut allocator);
    backing.reset();
    writer.allocate_new_page();
    // Two allocator-control writes, one complete prefix and one predecessor link.
    assert_eq!(backing.counts().writes, 4);
    assert_eq!(writer.overflows, [freed]);
    let mut bytes = [0; 128];
    backing.read(freed.get(), &mut bytes);
    assert_eq!(&bytes[..3], OVERFLOW_MAGIC);
    assert_eq!(&bytes[3..11], &NULL.get().to_le_bytes());
    assert_eq!(&bytes[11..], &[0xa5; 117]);
    assert_eq!(
        crate::engine::read_u64(&backing, address + OVERFLOW_ADDRESS_OFFSET),
        freed.get(),
    );
}
