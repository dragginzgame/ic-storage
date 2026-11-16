//! Native image tests. Fixture bytes predate the decoder and come from the
//! frozen layout's independent table assembly, not its descriptor generator.

extern crate std;

use std::{format, string::String, vec, vec::Vec};

use sha2::{Digest, Sha256};

use super::{AdmissionError as E, ObjectReference, Summary, inspect};

const META: usize = 8 * 1024 * 1024;
const PAGE: usize = 65536;
const ARENA: usize = META + PAGE;
const EMPTY: &[u8; PAGE] = include_bytes!("fixtures/empty-metadata.bin");
const NODE: &[u8; 16] = b"op.m0.node.v1\0\0\0";
const COUNTER: &[u8; 16] = b"op.m0.counter.v1";

fn put32(image: &mut [u8], at: usize, value: u32) {
    image[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(image: &mut [u8], at: usize, value: u64) {
    image[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

fn empty(pages: usize) -> Vec<u8> {
    let mut image = vec![0; (129 + pages) * PAGE];
    image[META..ARENA].copy_from_slice(EMPTY);
    put64(&mut image, META + 64, (pages * PAGE) as u64);
    seal(&mut image);
    image
}

// Deliberately independent of the decoder: mutations are resealed so tests
// cannot pass merely because a checksum mismatch hides invalid structure.
fn seal(image: &mut [u8]) {
    image[META + 160..META + 192].fill(0);
    let mut hash = Sha256::new();
    hash.update(b"ic-storage-new/m0/metadata/v1\0");
    hash.update(&image[META..ARENA]);
    image[META + 160..META + 192].copy_from_slice(&hash.finalize());
}

fn indexed(image: &mut [u8], entry: usize, offset: usize, schema: &[u8; 16], payload: u64) {
    let index = META + 1024 + entry * 32;
    put64(image, index, offset as u64);
    put64(image, index + 8, entry as u64 + 1);
    put64(image, index + 16, 48 + payload);
    let at = ARENA + offset;
    put64(image, at, entry as u64 + 1);
    image[at + 8..at + 24].copy_from_slice(schema);
    put64(image, at + 24, payload);
}

fn pair(image: &mut [u8], at: usize, offset: u64, identity: u64) {
    put64(image, at, offset);
    put64(image, at + 8, identity);
}

fn graph() -> Vec<u8> {
    let mut image = empty(1);
    indexed(&mut image, 0, 0, NODE, 24);
    indexed(&mut image, 1, 72, COUNTER, 40);
    put64(&mut image, META + 72, 160);
    put64(&mut image, META + 80, 3);
    put32(&mut image, META + 88, 2);
    pair(&mut image, META + 544, 72, 2);
    put64(&mut image, ARENA + 48, 7);
    pair(&mut image, ARENA + 56, 0, 1);
    put64(&mut image, ARENA + 72 + 48, 41);
    pair(&mut image, ARENA + 72 + 56, 0, 1);
    pair(&mut image, ARENA + 72 + 72, 0, 1);
    seal(&mut image);
    image
}

#[test]
fn retained_admission_excludes_transients_and_keeps_absolute_error_offsets() {
    use super::inspect_retained;
    let mut image = graph();
    image[..META].fill(0xcc);
    assert_eq!(inspect_retained(&image[META..]), inspect(&image));
    image[ARENA + 32] = 1;
    assert_eq!(
        inspect_retained(&image[META..]),
        Err(E::Reserved { at: ARENA + 32 })
    );
    assert_eq!(inspect_retained(&image[META..]), inspect(&image));
    assert_eq!(inspect_retained(&image), Err(E::MemorySize));
    assert_eq!(
        inspect_retained(&image[META..META + PAGE - 1]),
        Err(E::MemorySize)
    );
}

#[test]
fn public_mutation_api_reproduces_independent_graph_bytes() {
    use super::image::Image;
    let mut memory = vec![0; 130 * PAGE];
    let mut image = Image::initialize_fresh(&mut memory[META..], 129).unwrap();
    image
        .with_update(|s| {
            s.grow(1).unwrap();
            let node = s.allocate_node(7, None).unwrap();
            s.set_node_next(node, Some(node)).unwrap();
            let root = s.allocate_counter(41, Some(node), Some(node)).unwrap();
            s.publish_root(root).unwrap();
        })
        .unwrap();
    assert_eq!(image.bytes(), &graph()[META..]);
}

#[test]
fn canonical_empty_vector_matches_both_frozen_digests() {
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(
        hex(&EMPTY[160..192]),
        "944e725d1226789882410df6dbff62649034171bc5bdace98d8d72732049a5fa"
    );
    assert_eq!(
        hex(&Sha256::digest(EMPTY)),
        "62820b481cd4519884d2283fef29a53abb4cc0b49516c9a63cb5077300125a33"
    );
    let image = empty(0);
    assert_eq!(&image[META..ARENA], EMPTY);
    assert_eq!(
        inspect(&image),
        Ok(Summary {
            object_count: 0,
            committed_bytes: 0,
            used_bytes: 0,
            next_identity: 1,
            root: None
        })
    );
}

#[test]
fn sharing_cycle_offset_zero_and_scalar_mutation_are_valid() {
    let mut image = graph();
    let expected = Summary {
        object_count: 2,
        committed_bytes: PAGE as u64,
        used_bytes: 160,
        next_identity: 3,
        root: Some(ObjectReference {
            offset: 72,
            identity: 2,
        }),
    };
    assert_eq!(inspect(&image), Ok(expected));
    // Payload-only changes do not alter metadata integrity. The inspector is
    // representation admission, not a business-rule validator.
    put64(&mut image, ARENA + 48, u64::MAX);
    put64(&mut image, ARENA + 72 + 48, 0);
    assert_eq!(inspect(&image), Ok(expected));
    // Transient and unallocated arena bytes do not participate in admission.
    image[0] = 255;
    image[ARENA + 4096] = 255;
    assert_eq!(inspect(&image), Ok(expected));
}

#[test]
fn physical_extent_and_accounting_are_checked_before_index_access() {
    for length in [0, ARENA - 1, ARENA - PAGE, ARENA + 1, 257 * PAGE] {
        assert_eq!(inspect(&vec![0; length]), Err(E::MemorySize));
    }
    for (offset, value) in [
        (64, 0),
        (64, u64::MAX),
        (72, u64::MAX),
        (72, 159),
        (72, 152),
        (72, 168),
        (80, 0),
        (80, u64::MAX),
        (88, 1025),
        (88, u64::from(u32::MAX)),
    ] {
        let mut image = graph();
        put64(&mut image, META + offset, value);
        // The count is u32; do not overwrite its adjacent capacity constant.
        if offset == 88 {
            put32(&mut image, META + 92, 1024);
        }
        seal(&mut image);
        assert!(
            inspect(&image).is_err(),
            "accounting field {offset}, {value}"
        );
    }
    let mut expanded = graph();
    expanded.resize(131 * PAGE, 0);
    assert_eq!(inspect(&expanded), Err(E::Accounting));
    put64(&mut expanded, META + 64, (2 * PAGE) as u64);
    seal(&mut expanded);
    assert_eq!(
        inspect(&expanded)
            .expect("committed growth")
            .committed_bytes,
        (2 * PAGE) as u64
    );
    let mut empty_cursor = empty(1);
    put64(&mut empty_cursor, META + 72, 8);
    seal(&mut empty_cursor);
    assert_eq!(inspect(&empty_cursor), Err(E::Accounting));
    let maximum = empty(127);
    assert_eq!(
        inspect(&maximum)
            .expect("maximum physical extent")
            .committed_bytes,
        8323072
    );
}

#[test]
fn all_constant_header_fields_and_root_ids_are_exact() {
    for offset in [
        0, 8, 12, 16, 32, 40, 48, 56, 92, 96, 100, 104, 108, 112, 116, 120, 124, 128, 132, 512, 528,
    ] {
        let mut image = empty(0);
        image[META + offset] ^= 1;
        seal(&mut image);
        assert_eq!(inspect(&image), Err(E::Format { at: META + offset }));
    }
    assert_eq!(inspect(&vec![0; ARENA]), Err(E::Format { at: META }));
}

#[test]
fn reserved_and_inactive_index_bytes_must_be_zero_even_with_valid_digest() {
    for offset in [
        136, 159, 192, 255, 560, 575, 576, 1023, 1088, 33791, 33792, 65535,
    ] {
        let mut image = graph();
        image[META + offset] = 1;
        seal(&mut image);
        assert_eq!(inspect(&image), Err(E::Reserved { at: META + offset }));
    }
    let mut image = graph();
    image[ARENA + 32] = 1;
    assert_eq!(inspect(&image), Err(E::Reserved { at: ARENA + 32 }));
}

#[test]
fn integrity_and_complete_descriptors_are_both_required() {
    for offset in [64, 72, 80, 88, 160, 191, 256, 544, 1024] {
        let mut image = graph();
        image[META + offset] ^= 1;
        assert_eq!(inspect(&image), Err(E::Integrity));
    }
    for offset in [
        256, 272, 276, 280, 284, 288, 292, 296, 300, 304, 320, 336, 352, 368, 384, 400, 404, 408,
        412, 416, 448, 464, 480, 511,
    ] {
        let mut image = graph();
        image[META + offset] ^= 1;
        seal(&mut image);
        assert_eq!(
            inspect(&image),
            Err(E::Descriptors),
            "descriptor byte {offset}"
        );
    }
}

#[test]
fn index_span_and_header_corruption_cannot_hide_behind_a_matching_checksum() {
    for (relative, value, entry) in [
        (0, 8, 0),
        (8, 0, 0),
        (16, 0, 0),
        (16, u64::MAX, 0),
        (24, 1, 0),
        (32, 64, 1),
        (40, 1, 1),
        (48, 96, 1),
        (48, u64::MAX - 7, 1), // Aligned span whose addition to offset 72 overflows.
    ] {
        let mut image = graph();
        put64(&mut image, META + 1024 + relative, value);
        seal(&mut image);
        assert_eq!(inspect(&image), Err(E::Index { entry }));
    }
    for offset in [0, 8, 24, 72, 80, 96] {
        let mut image = graph();
        image[ARENA + offset] ^= 1;
        assert_eq!(
            inspect(&image),
            Err(E::Object {
                entry: u32::from(offset >= 72)
            })
        );
    }
}

#[test]
fn wrong_reference_id_offset_and_target_schema_are_rejected() {
    for (at, offset, identity) in [
        (ARENA + 56, 8, 1),
        (ARENA + 56, 8, 0),
        (ARENA + 56, 0, 3),
        (ARENA + 56, 0, u64::MAX),
        (ARENA + 56, u64::MAX, 1),
        (ARENA + 56, 72, 2),
        (ARENA + 128, 0, 2),
        (META + 544, 0, 1),
        (META + 544, 72, 0),
    ] {
        let mut image = graph();
        pair(&mut image, at, offset, identity);
        seal(&mut image);
        assert_eq!(inspect(&image), Err(E::Reference { at }));
    }
    let mut image = graph();
    pair(&mut image, ARENA + 56, 0, 0);
    assert!(inspect(&image).is_ok(), "null Node.next is valid");
}

#[test]
fn unreachable_objects_are_validated_and_null_root_does_not_initialize_them() {
    let mut image = graph();
    pair(&mut image, META + 544, 0, 0);
    seal(&mut image);
    assert_eq!(inspect(&image).expect("construction state").root, None);
    pair(&mut image, ARENA + 56, 123, 1);
    assert_eq!(inspect(&image), Err(E::Reference { at: ARENA + 56 }));
}

#[test]
fn maximum_index_and_forward_cycle_need_no_recursive_traversal() {
    let mut image = empty(2);
    for i in 0..1024 {
        indexed(&mut image, i, i * 72, NODE, 24);
        let next = (i + 1) % 1024;
        pair(
            &mut image,
            ARENA + i * 72 + 56,
            (next * 72) as u64,
            next as u64 + 1,
        );
    }
    put32(&mut image, META + 88, 1024);
    put64(&mut image, META + 80, 1025);
    put64(&mut image, META + 72, 1024 * 72);
    seal(&mut image);
    assert_eq!(
        inspect(&image).expect("bounded forward cycle").object_count,
        1024
    );
}

#[test]
fn inspection_never_changes_input_on_success_or_failure() {
    let mut image = graph();
    let before = Sha256::digest(&image);
    assert!(inspect(&image).is_ok());
    assert_eq!(Sha256::digest(&image), before);
    image[META] = 0;
    let before = Sha256::digest(&image);
    assert!(inspect(&image).is_err());
    assert_eq!(Sha256::digest(&image), before);
}
