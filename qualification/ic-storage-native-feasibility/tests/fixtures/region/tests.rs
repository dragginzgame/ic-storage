//! Host backing supplies actual allocation authority; descriptors do not.

use super::{Descriptor, Error, Region};
use std::{
    alloc::{Allocator, Layout},
    mem::{self, MaybeUninit},
    ptr::NonNull,
};

const SENTINEL: u64 = 0xfedc_ba98_7654_3210;

#[test]
fn spare_capacity_need_not_be_initialized() {
    let mut backing = [MaybeUninit::<u64>::uninit(); 16];
    backing[0].write(17);
    // SAFETY: this local exclusively owns all 16 aligned slots until the
    // controller ends. Only the promised first word is initialized.
    #[expect(
        unsafe_code,
        reason = "test the minimal initialized-prefix loan contract"
    )]
    let region = unsafe {
        Region::claim(
            NonNull::from(&mut backing).cast(),
            16,
            Descriptor {
                length: 1,
                capacity: 4,
            },
        )
    }
    .unwrap();
    let mut values = region.restore().unwrap();
    assert_eq!(values.as_slice(), &[17]);
    values.push(42);
    values.try_reserve_exact(6).unwrap();
    assert_eq!(values.as_slice(), &[17, 42]);
    assert_eq!(region.release(values).unwrap().length, 2);
}

#[test]
fn unsupported_geometry_refuses_before_issuance() {
    let mut backing = [MaybeUninit::<u64>::uninit(); 513];
    for words in [0, 513] {
        // SAFETY: the actual allocation covers even the rejected extent. The
        // empty root promises no initialized words, and no client is created.
        #[expect(
            unsafe_code,
            reason = "exercise extent refusal with valid backing custody"
        )]
        let result = unsafe {
            Region::claim(
                NonNull::from(&mut backing).cast(),
                words,
                Descriptor {
                    length: 0,
                    capacity: 0,
                },
            )
        };
        assert!(matches!(result, Err(Error::Geometry)));
    }
}

struct Backing([MaybeUninit<u64>; 16]);

impl Backing {
    fn new() -> Self {
        Self([MaybeUninit::new(SENTINEL); 16])
    }

    fn claim(&mut self, length: usize, capacity: usize) -> Result<Region<'_>, Error> {
        let base = NonNull::from(&mut self.0).cast::<u8>();
        // SAFETY: the exclusive borrow ties the controller lifetime to this
        // stable backing. Every slot starts initialized and all test writers
        // write u64 or zero bytes. No previous client can outlive this borrow.
        #[expect(
            unsafe_code,
            reason = "test owner supplies exclusive initialized live backing"
        )]
        unsafe {
            Region::claim(base, self.0.len(), Descriptor { length, capacity })
        }
    }

    fn words(&self) -> [u64; 16] {
        // SAFETY: all slots were initialized at construction and these tests
        // never write uninitialized data. This borrow excludes a live controller.
        #[expect(
            unsafe_code,
            reason = "inspect initialized test backing outside its loan"
        )]
        self.0.map(|word| unsafe { word.assume_init() })
    }
}

#[test]
fn fresh_issuance_preserves_payload_and_replaces_controller() {
    let mut backing = Backing::new();
    let root = {
        let source = backing.claim(3, 4).unwrap();
        let mut values = source.restore().unwrap();
        assert_eq!(values.as_slice(), &[SENTINEL; 3]);
        values[1] = 17;
        values.push(42);
        let root = source.release(values).unwrap();
        assert_eq!(
            root,
            Descriptor {
                length: 4,
                capacity: 4
            }
        );
        assert!(source.restore().is_err());
        assert!(source.allocate(Layout::new::<u64>()).is_err());
        root
    };
    assert_eq!(&backing.words()[4..], &[SENTINEL; 12]);
    {
        let destination = backing.claim(root.length, root.capacity).unwrap();
        let mut values = destination.restore().unwrap();
        assert_eq!(values.as_slice(), &[SENTINEL, 17, SENTINEL, 42]);
        values.try_reserve_exact(1).unwrap();
        values.push(97);
        assert_eq!(values[4], 97);
    }
    assert_eq!(&backing.words()[5..], &[SENTINEL; 11]);
}

#[test]
fn reservation_duplicate_restore_and_invalid_roots_refuse_without_writes() {
    let mut backing = Backing::new();
    for (length, capacity) in [(1, 0), (4, 3), (0, 17), (17, 17)] {
        assert!(matches!(backing.claim(length, capacity), Err(Error::Root)));
        assert_eq!(backing.words(), [SENTINEL; 16]);
    }
    {
        let region = backing.claim(2, 4).unwrap();
        let layout = Layout::array::<u64>(4).unwrap();
        assert!(region.allocate(layout).is_err());
        assert!(region.allocate_zeroed(layout).is_err());
        let values = region.restore().unwrap();
        assert!(matches!(region.restore(), Err(Error::State)));
        assert!(region.allocate(layout).is_err());
        assert_eq!(values.as_slice(), &[SENTINEL; 2]);
        drop(values);
        // Dropping the root frees its block but cannot replay the old descriptor.
        assert!(matches!(region.restore(), Err(Error::State)));
    }
    assert_eq!(backing.words(), [SENTINEL; 16]);
}

#[test]
fn vector_grows_shrinks_removes_and_reuses_without_moving_backing() {
    let mut backing = Backing::new();
    let region = backing.claim(2, 2).unwrap();
    let mut values = region.restore().unwrap();
    let base = values.as_ptr();
    values.try_reserve_exact(14).unwrap();
    assert_eq!(values.capacity(), 16);
    assert_eq!(values.as_ptr(), base);
    values.extend(2..16);
    let before = values.to_vec();
    assert!(values.try_reserve_exact(1).is_err());
    assert_eq!(values.as_slice(), before);
    assert_eq!(values.capacity(), 16);
    assert_eq!(values.as_ptr(), base);
    assert_eq!(values.remove(3), 3);
    values.truncate(3);
    values.shrink_to_fit();
    assert_eq!(values.capacity(), 3);
    assert_eq!(values.as_ptr(), base);
    assert_eq!(values.as_slice(), &[SENTINEL, SENTINEL, 2]);
    values.clear();
    values.shrink_to_fit();
    assert_eq!(values.capacity(), 0);
    values.try_reserve_exact(2).unwrap();
    values.push(123);
    assert_eq!(values.as_ptr(), base);
    drop(values);
    let mut replacement = Vec::new_in(&region);
    replacement.try_reserve_exact(16).unwrap();
    replacement.push(456);
    assert_eq!(replacement.as_ptr(), base);
    assert_eq!(replacement[0], 456);
}

#[test]
fn empty_and_cleared_roots_release_once_and_forgetting_quarantines() {
    for capacity in [0, 4] {
        let mut backing = Backing::new();
        let region = backing.claim(0, capacity).unwrap();
        let values = region.restore().unwrap();
        assert!(values.is_empty());
        assert_eq!(
            region.release(values).unwrap(),
            Descriptor {
                length: 0,
                capacity
            }
        );
        assert!(region.restore().is_err());
        assert!(region.allocate(Layout::new::<u64>()).is_err());
    }
    let mut backing = Backing::new();
    let region = backing.claim(2, 4).unwrap();
    let values = region.restore().unwrap();
    mem::forget(values);
    assert!(region.restore().is_err());
    assert!(region.allocate(Layout::new::<u64>()).is_err());
    let empty = Vec::new_in(&region);
    assert!(region.release(empty).is_err());
}

#[test]
fn wrong_controller_release_preserves_the_original_owner() {
    let mut first = Backing::new();
    let mut second = Backing::new();
    let first = first.claim(1, 4).unwrap();
    let second = second.claim(0, 0).unwrap();
    let values = first.restore().unwrap();
    let (error, mut values) = second.release(values).unwrap_err();
    assert_eq!(error, Error::State);
    values[0] = 77;
    assert_eq!(first.release(values).unwrap().length, 1);
}

#[test]
fn unsupported_layouts_and_failed_growth_preserve_the_live_block() {
    let mut backing = Backing::new();
    let region = backing.claim(0, 0).unwrap();
    drop(region.restore().unwrap());
    for layout in [
        Layout::from_size_align(8, 1).unwrap(),
        Layout::from_size_align(16, 16).unwrap(),
        Layout::from_size_align(9, 8).unwrap(),
        Layout::from_size_align(0, 8).unwrap(),
        Layout::array::<u64>(17).unwrap(),
    ] {
        assert!(region.allocate(layout).is_err());
    }
    let layout = Layout::array::<u64>(2).unwrap();
    let block = region.allocate_zeroed(layout).unwrap().cast::<u64>();
    // SAFETY: this exact allocator issued and zeroed both words. The pointer
    // and old layout identify its sole live block; failed growth leaves it live.
    #[expect(
        unsafe_code,
        reason = "exercise Allocator operations under their actual caller contract"
    )]
    unsafe {
        assert_eq!(block.as_ptr().read(), 0);
        assert_eq!(block.as_ptr().add(1).read(), 0);
        block.as_ptr().write(55);
        assert!(
            region
                .grow(block.cast(), layout, Layout::array::<u64>(17).unwrap())
                .is_err()
        );
        assert!(
            region
                .grow_zeroed(block.cast(), layout, Layout::array::<u64>(3).unwrap())
                .is_err()
        );
        assert_eq!(block.as_ptr().read(), 55);
        region.deallocate(block.cast(), layout);
    }
    let mut values = Vec::new_in(&region);
    values.try_reserve_exact(16).unwrap();
    values.push(91_u64);
    assert_eq!(values[0], 91);
}

#[test]
fn scoped_core_allocation_refusal_preserves_prior_mutation_and_custody() {
    let mut backing = Backing::new();
    let region = backing.claim(16, 16).unwrap();
    let mut values = region.restore().unwrap();
    let pointer = values.as_ptr();
    let result = ic_storage_native::with_vector(&mut values, 17, |mut view| {
        view.values_mut()[0] = 41;
        // The logical limit permits this, but the real backing has only 16 slots.
        view.push(99)
    });
    assert_eq!(result, Ok(Err(ic_storage_native::Refusal::Allocation)));
    assert_eq!(values.len(), 16);
    assert_eq!(values.capacity(), 16);
    assert_eq!(values.as_ptr(), pointer);
    assert_eq!(values[0], 41);
    assert_eq!(values[1..], [SENTINEL; 15]);
    assert_eq!(
        region.release(values).unwrap(),
        Descriptor {
            length: 16,
            capacity: 16
        }
    );
}
