//! Fresh V3 controllers borrow one original host allocation from its owner.
//! Metadata is checked against live provider authority, not trusted from bytes.

use super::super::{Controller, Descriptor, Error};
use ic_storage_native::record::Field;
use ic_storage_native_feasibility::record::narrow::{Error as RecordError, V3};
use std::{mem::MaybeUninit, ptr::NonNull};

const SLOTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Schema {
    id: [u8; 8],
    version: u32,
    size: usize,
    align: usize,
    fields: [Field; 2],
}

impl Schema {
    const CURRENT: Self = Self {
        id: *V3::STORE_ID,
        version: V3::STORE_VERSION,
        size: V3::STORE_SIZE,
        align: V3::STORE_ALIGN,
        fields: V3::STORE_FIELDS,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Metadata {
    schema: Schema,
    root: Descriptor,
}

struct Provider {
    backing: Box<[MaybeUninit<V3>; SLOTS]>,
    metadata: Metadata,
    available: bool,
}

impl Provider {
    fn new(values: &[V3], capacity: usize) -> Self {
        assert!(values.len() <= capacity && capacity <= SLOTS);
        let mut backing = Box::new([MaybeUninit::uninit(); SLOTS]);
        for (slot, value) in backing.iter_mut().zip(values) {
            slot.write(*value);
        }
        Self {
            backing,
            metadata: Metadata {
                schema: Schema::CURRENT,
                root: Descriptor {
                    length: values.len(),
                    capacity,
                },
            },
            available: true,
        }
    }

    // A trusted fixture operation owns mutation policy. This is not a public
    // collection interface. Its result cannot carry a controller-bound owner.
    fn run<R>(
        &mut self,
        requested: Metadata,
        operation: impl for<'region, 'backing> FnOnce(
            &mut Vec<V3, &'region Controller<'backing, V3>>,
        ) -> R,
    ) -> Result<R, Error> {
        if !self.available {
            return Err(Error::State);
        }
        if requested.schema != Schema::CURRENT || requested != self.metadata {
            return Err(Error::Root);
        }
        self.available = false;
        let (result, root) = {
            // SAFETY: the private provider owns this original Box and lends it
            // exclusively until the controller and clients end. Its metadata
            // comes only from fresh initialization or successful prior release.
            // It witnesses the valid V3 prefix; unused slots and record padding
            // need not be initialized. Schema/extent admission precedes this call.
            #[expect(
                unsafe_code,
                reason = "exclusive original host backing with V3 initialization witness"
            )]
            let region = unsafe {
                Controller::<V3>::claim(
                    NonNull::from(self.backing.as_mut()).cast(),
                    SLOTS,
                    requested.root,
                )
            }?;
            let mut values = region.restore()?;
            let result = operation(&mut values);
            let root = region.release(values).map_err(|(error, values)| {
                drop(values);
                error
            })?;
            (result, root)
        };
        self.metadata.root = root;
        self.available = true;
        Ok(result)
    }
}

fn value(counter: u32) -> V3 {
    V3::new(counter, 100).unwrap()
}

#[test]
fn fresh_controllers_preserve_record_values_and_grow_in_place() {
    for (length, capacity) in [(0, 0), (0, 4), (3, 4), (16, 16)] {
        let initial: Vec<_> = (0..length).map(|i| value(i as u32)).collect();
        let mut provider = Provider::new(&initial, capacity);
        let address = provider.backing.as_ptr();
        provider
            .run(provider.metadata, |values| {
                assert_eq!(values.as_slice(), initial);
                assert_eq!(values.capacity(), capacity);
                if capacity != 0 {
                    assert_eq!(values.as_ptr().cast::<MaybeUninit<V3>>(), address);
                }
                values.clear();
                values.try_reserve_exact(SLOTS).unwrap();
                values.extend((0..SLOTS).map(|i| value(i as u32)));
                assert_eq!(values.as_ptr().cast::<MaybeUninit<V3>>(), address);
                assert!(values.try_reserve_exact(1).is_err());
                assert_eq!(values.len(), SLOTS);
                values[0].store_with(|mut view| view.add(7)).unwrap();
            })
            .unwrap();
        // The previous allocator and owning Vec ended; this call creates both afresh.
        provider
            .run(provider.metadata, |values| {
                assert_eq!(values[0], value(7));
                assert_eq!(values.len(), SLOTS);
                assert_eq!(values.remove(1), value(1));
                values.truncate(2);
                values.shrink_to_fit();
                assert_eq!(values.capacity(), 2);
                assert_eq!(values.as_slice(), [value(7), value(2)]);
            })
            .unwrap();
        provider
            .run(provider.metadata, |values| {
                assert_eq!(values.as_slice(), [value(7), value(2)]);
                values.clear();
                values.shrink_to_fit();
                assert_eq!(values.capacity(), 0);
                values.push(value(9));
            })
            .unwrap();
        provider
            .run(provider.metadata, |values| {
                assert_eq!(values.as_slice(), [value(9)])
            })
            .unwrap();
    }
}

#[test]
fn schema_and_extent_refusal_precedes_claim_and_preserves_provider_authority() {
    let mut provider = Provider::new(&[value(1), value(2)], 4);
    let original = provider.metadata;
    for change in [
        (|m: &mut Metadata| m.schema.version = 2) as fn(&mut Metadata),
        |m| m.schema.id[0] ^= 1,
        |m| m.schema.size = 8,
        |m| m.schema.align = 4,
        |m| m.schema.fields[0].scalar = ic_storage_native::record::Scalar::U64,
        |m| m.root.length = 3, // Within capacity, but outside initialized authority.
        |m| m.root.length = usize::MAX,
        |m| m.root.capacity = SLOTS + 1,
        |m| {
            m.root = Descriptor {
                length: 0,
                capacity: 0,
            }
        },
    ] {
        let mut requested = original;
        change(&mut requested);
        assert_eq!(
            provider.run(requested, |_| panic!("refused callback ran")),
            Err(Error::Root)
        );
        assert!(provider.available);
        assert_eq!(provider.metadata, original);
    }
    provider
        .run(original, |values| {
            assert_eq!(values.as_slice(), [value(1), value(2)])
        })
        .unwrap();
}

#[test]
fn ordinary_errors_publish_prior_record_mutations_and_forgotten_views_do_not_block() {
    let mut provider = Provider::new(&[value(1)], 1);
    assert_eq!(
        provider.run(provider.metadata, |values| {
            values[0].store_with(|mut view| {
                view.add(2)?;
                view.add(100)
            })
        }),
        Ok(Err(RecordError::Ceiling))
    );
    provider
        .run(provider.metadata, |values| {
            assert_eq!(values[0], value(3));
            values[0].store_with(|view| {
                #[expect(
                    clippy::forget_non_drop,
                    reason = "forgotten scalar view has no owner obligation"
                )]
                std::mem::forget(view);
            });
        })
        .unwrap();
    assert!(provider.available);
    provider
        .run(provider.metadata, |values| values.push(value(4)))
        .unwrap();
}

#[test]
fn lost_record_owner_blocks_provider_reentry_without_guard_cleanup() {
    let mut provider = Provider::new(&[value(1)], 4);
    let metadata = provider.metadata;
    assert_eq!(
        provider.run(metadata, |values| {
            let replacement = Vec::new_in(*values.allocator());
            let lost = std::mem::replace(values, replacement);
            std::mem::forget(lost);
        }),
        Err(Error::State)
    );
    assert!(!provider.available);
    assert_eq!(provider.metadata, metadata);
    assert_eq!(
        provider.run(metadata, |_| panic!("lost owner reentered")),
        Err(Error::State)
    );
}

#[test]
fn unwinding_blocks_reentry_and_generated_logical_payloads_restore_after_teardown() {
    let payloads = {
        let mut provider = Provider::new(&[value(7), value(8)], 4);
        let payloads = provider
            .run(provider.metadata, |values| {
                values
                    .iter()
                    .map(|v| v.store_encode().unwrap())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let metadata = provider.metadata;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            provider.run(metadata, |values| {
                values[0].store_with(|mut view| view.add(1)).unwrap();
                values.push(value(9));
                panic!("after mutation");
            })
        }));
        assert!(panic.is_err());
        assert!(!provider.available);
        assert_eq!(
            provider.run(metadata, |_| panic!("poisoned provider reentered")),
            Err(Error::State)
        );
        payloads
    };
    // Only generated logical fields cross this boundary; this is not a new wire
    // envelope or governed checkpoint-provider implementation.
    let values: Vec<_> = payloads
        .iter()
        .map(|bytes| V3::store_decode(bytes).unwrap())
        .collect();
    let mut restored = Provider::new(&values, values.len());
    restored
        .run(restored.metadata, |values| {
            assert_eq!(values.as_slice(), [value(7), value(8)]);
            values[0].store_with(|mut view| view.add(1)).unwrap();
            values.push(value(9));
        })
        .unwrap();
    restored
        .run(restored.metadata, |values| {
            assert_eq!(values.as_slice(), [value(8), value(8), value(9)])
        })
        .unwrap();
}
