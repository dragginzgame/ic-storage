use super::*;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};

#[derive(Clone, Default)]
struct Model {
    bytes: Rc<RefCell<Vec<u8>>>,
    reads: Rc<Cell<usize>>,
    writes: Rc<Cell<usize>>,
    refuse_growth: Rc<Cell<bool>>,
}

impl Memory for Model {
    fn size(&self) -> u64 {
        self.bytes.borrow().len() as u64 / PAGE_SIZE
    }
    fn grow(&self, pages: u64) -> Option<u64> {
        let old = self.size();
        let size = old.checked_add(pages)?.checked_mul(PAGE_SIZE)?;
        if self.refuse_growth.get() || size > 1_048_576 {
            return None;
        }
        self.bytes.borrow_mut().resize(size as usize, 0);
        Some(old)
    }
    fn read(&self, offset: u64, destination: &mut [u8]) {
        self.reads.set(self.reads.get() + destination.len());
        destination.copy_from_slice(
            &self.bytes.borrow()[offset as usize..offset as usize + destination.len()],
        );
    }
    fn write(&self, offset: u64, source: &[u8]) {
        self.writes.set(self.writes.get() + source.len());
        self.bytes.borrow_mut()[offset as usize..offset as usize + source.len()]
            .copy_from_slice(source);
    }
}

const SPECS: [Specification; 2] = [
    Specification { id: 1, bytes: 32 },
    Specification { id: 2, bytes: 48 },
];

#[test]
fn independent_reopening_preserves_records_and_bounded_page_io() {
    let memory = Model::default();
    {
        let mut store = Store::create(memory.clone(), &SPECS).unwrap();
        let mut record = Record::create(store.take(1).unwrap(), [4; 8]).unwrap();
        let mut pages = Pages::<8, _>::create(store.take(2).unwrap()).unwrap();
        record.set([7; 8]).unwrap();
        pages.push(&[1; 8]).unwrap();
        pages.push(&[2; 8]).unwrap();
        pages.push(&[3; 8]).unwrap();
        let before = memory.bytes.borrow().clone();
        assert_eq!(pages.push(&[4; 8]), Err(Error::Bounds));
        assert_eq!(pages.set(3, &[4; 8]), Err(Error::Bounds));
        assert_eq!(*memory.bytes.borrow(), before);
        pages.set(1, &[9; 8]).unwrap();
    }
    memory.writes.set(0);
    let mut store = Store::open(memory.clone(), &SPECS).unwrap();
    let record = Record::<[u8; 8], _>::load(store.take(1).unwrap(), 8).unwrap();
    let mut pages = Pages::<8, _>::load(store.take(2).unwrap()).unwrap();
    assert_eq!(record.get(), &[7; 8]);
    assert_eq!(memory.writes.get(), 0);
    memory.reads.set(0);
    assert_eq!(pages.get(1), Some([9; 8]));
    assert_eq!(memory.reads.get(), 8);
    assert_eq!(pages.get(3), None);
    assert_eq!(memory.reads.get(), 8);
    assert_eq!(pages.pop(), Some([3; 8]));
    pages.clear();
    pages.push(&[8; 8]).unwrap();
    assert_eq!(pages.get(0), Some([8; 8]));
}

#[test]
fn existing_unknown_or_corrupt_storage_never_initializes_or_repairs() {
    let memory = Model::default();
    assert!(matches!(
        Store::open(memory.clone(), &SPECS),
        Err(Error::Layout)
    ));
    assert_eq!(memory.size(), 0);
    drop(Store::create(memory.clone(), &SPECS).unwrap());
    let original = memory.bytes.borrow().clone();
    memory.writes.set(0);
    assert!(matches!(
        Store::create(memory.clone(), &SPECS),
        Err(Error::NotFresh)
    ));
    for offset in 0..HEADER {
        memory.bytes.borrow_mut()[offset] ^= 1;
        assert!(matches!(
            Store::open(memory.clone(), &SPECS),
            Err(Error::Layout)
        ));
        memory.bytes.borrow_mut()[offset] ^= 1;
    }
    assert_eq!(memory.writes.get(), 0);
    assert_eq!(*memory.bytes.borrow(), original);
    let mut opened = Store::open(memory.clone(), &SPECS).unwrap();
    assert!(matches!(
        Record::create(opened.take(1).unwrap(), [0; 8]),
        Err(Error::NotFresh)
    ));
    assert_eq!(memory.writes.get(), 0);
    drop(opened);
    memory.grow(1).unwrap();
    assert!(matches!(
        Store::open(memory.clone(), &SPECS),
        Err(Error::Layout)
    ));
    assert_eq!(memory.writes.get(), 0);
    memory.bytes.borrow_mut().clear();
    assert!(matches!(Store::open(memory, &SPECS), Err(Error::Layout)));
}

#[test]
fn invalid_layouts_and_failed_growth_have_no_writes() {
    let memory = Model::default();
    for specs in [
        &[][..],
        &[Specification { id: 0, bytes: 16 }][..],
        &[Specification { id: 1, bytes: 15 }][..],
        &[Specification {
            id: 1,
            bytes: u64::MAX,
        }][..],
        &[SPECS[0], SPECS[0]][..],
        &[SPECS[1], SPECS[0]][..],
        &[SPECS[0]; 5][..],
    ] {
        assert!(matches!(
            Store::create(memory.clone(), specs),
            Err(Error::Bounds)
        ));
        assert_eq!(memory.size(), 0);
    }
    memory.refuse_growth.set(true);
    assert!(matches!(
        Store::create(memory.clone(), &SPECS),
        Err(Error::Growth)
    ));
    assert_eq!(memory.size(), 0);
    assert_eq!(memory.writes.get(), 0);
}

#[test]
fn forgotten_claims_are_not_reissued_and_identifiers_are_exact() {
    let memory = Model::default();
    let mut store = Store::create(memory.clone(), &SPECS).unwrap();
    assert!(matches!(store.take(99), Err(Error::Bounds)));
    core::mem::forget(store.take(1).unwrap());
    assert!(matches!(store.take(1), Err(Error::Claimed)));
    assert!(matches!(
        Store::open(memory.clone(), &SPECS[..1]),
        Err(Error::Layout)
    ));
    let different = [
        Specification { id: 3, ..SPECS[0] },
        Specification { id: 4, ..SPECS[1] },
    ];
    assert!(matches!(
        Store::open(memory, &different),
        Err(Error::Layout)
    ));
}

#[test]
fn framing_lengths_widths_and_logical_limits_refuse_without_writes() {
    let memory = Model::default();
    let mut store = Store::create(memory.clone(), &SPECS).unwrap();
    let mut record = Record::create(store.take(1).unwrap(), alloc::vec![1; 8]).unwrap();
    let mut pages = Pages::<8, _>::create(store.take(2).unwrap()).unwrap();
    pages.push(&[2; 8]).unwrap();
    let before = memory.bytes.borrow().clone();
    assert_eq!(record.set(alloc::vec![0; 17]), Err(Error::Bounds));
    assert_eq!(*memory.bytes.borrow(), before);
    drop((record, pages, store));
    for limit in [0, 7] {
        let mut store = Store::open(memory.clone(), &SPECS).unwrap();
        assert!(matches!(
            Record::<Vec<u8>, _>::load(store.take(1).unwrap(), limit),
            Err(Error::Bounds)
        ));
    }
    let mut store = Store::open(memory.clone(), &SPECS).unwrap();
    assert!(matches!(
        Record::<[u8; 9], _>::load(store.take(1).unwrap(), 16),
        Err(Error::Bounds)
    ));
    assert!(matches!(
        Pages::<4, _>::load(store.take(2).unwrap()),
        Err(Error::Layout)
    ));
    memory.writes.set(0);
    for offset in [
        HEADER,
        HEADER + 8,
        HEADER + 32,
        HEADER + 32 + 8,
        HEADER + 32 + 16,
    ] {
        memory.bytes.borrow_mut()[offset] = 255;
        let mut store = Store::open(memory.clone(), &SPECS).unwrap();
        if offset < HEADER + 32 {
            assert!(Record::<Vec<u8>, _>::load(store.take(1).unwrap(), 16).is_err());
        } else {
            assert!(Pages::<8, _>::load(store.take(2).unwrap()).is_err());
        }
        memory.bytes.borrow_mut().copy_from_slice(&before);
    }
    assert_eq!(memory.writes.get(), 0);
}
