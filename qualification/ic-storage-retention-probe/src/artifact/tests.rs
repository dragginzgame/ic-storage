use super::*;

#[test]
fn evidence_hashes_preserve_original_bytes_and_lowercase_text() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(&sha256(b"abc")[..4], &[0xba, 0x78, 0x16, 0xbf]);
    assert_ne!(sha256(b"abc"), sha256(b"abc\0"));
}

#[test]
fn module_byte_limits_and_core_encoding_are_admitted_before_validation() {
    let module = b"\0asm\x01\0\0\0";
    validate_wasm(module, module.len()).unwrap();
    assert!(matches!(
        validate_wasm(module, module.len() - 1),
        Err(Refusal::Limit)
    ));
    assert!(matches!(
        validate_wasm(b"\0asm\x0d\0\x01\0", 8),
        Err(Refusal::Encoding)
    ));
    assert!(matches!(
        validate_wasm(&module[..7], 8),
        Err(Refusal::Encoding)
    ));
    let mut invalid = module.to_vec();
    invalid.extend_from_slice(&[1, 5]);
    assert!(matches!(
        validate_wasm(&invalid, 100),
        Err(Refusal::Validation(_))
    ));
}

#[test]
fn text_bounds_and_instruction_validation_remain_separate() {
    let text = "(module)";
    assert_eq!(
        compile_wat(text, text.len(), 8).unwrap(),
        b"\0asm\x01\0\0\0"
    );
    assert!(matches!(
        compile_wat(text, text.len() - 1, 8),
        Err(Refusal::Limit)
    ));
    assert!(matches!(
        compile_wat(text, text.len(), 7),
        Err(Refusal::Limit)
    ));
    assert!(matches!(
        compile_wat("(module", 100, 100),
        Err(Refusal::Text(_))
    ));
    assert!(compile_wat("(component)", 100, 100).is_err());
    let invalid = compile_wat("(module (func (result i32) i64.const 0))", 100, 100).unwrap();
    assert!(matches!(
        validate_wasm(&invalid, 100),
        Err(Refusal::Validation(_))
    ));
}
