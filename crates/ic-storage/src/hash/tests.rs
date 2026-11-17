use super::*;

#[test]
fn standard_vectors_and_chunked_inputs_produce_identical_hashes() {
    assert_eq!(fnv64(FNV_OFFSET, b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv64(FNV_OFFSET, b"hello"), 0xa430_d846_80aa_bd0b);
    let mut streamed = FNV_OFFSET;
    for chunk in [b"he".as_slice(), b"", b"ll", b"o"] {
        streamed = fnv64(streamed, chunk);
    }
    assert_eq!(streamed, 0xa430_d846_80aa_bd0b);
}
