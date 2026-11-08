use super::*;

#[test]
fn byte_codec_borrows_and_preserves_every_encoding() {
    for value in u8::MIN..=u8::MAX {
        let bytes = value.to_bytes_checked().unwrap();
        assert!(matches!(bytes, Cow::Borrowed(_)));
        assert_eq!(bytes.as_ref(), &[value]);
        assert_eq!(u8::from_bytes(bytes).unwrap(), value);
        let owned = value.into_bytes_checked().unwrap();
        assert_eq!(owned, [value]);
        assert_eq!(u8::from_bytes(Cow::Owned(owned)).unwrap(), value);
    }
    for bytes in [&[][..], &[0, 1][..]] {
        assert_eq!(
            u8::from_bytes(Cow::Borrowed(bytes)),
            Err(CodecError::Invalid("integer width"))
        );
    }
}

#[test]
fn boolean_codec_borrows_and_rejects_non_boolean_bytes() {
    for (value, byte) in [(false, 0), (true, 1)] {
        let bytes = value.to_bytes_checked().unwrap();
        assert!(matches!(bytes, Cow::Borrowed(_)));
        assert_eq!(bytes.as_ref(), &[byte]);
        assert_eq!(bool::from_bytes(bytes).unwrap(), value);
        let owned = value.into_bytes_checked().unwrap();
        assert_eq!(owned, [byte]);
        assert_eq!(bool::from_bytes(Cow::Owned(owned)).unwrap(), value);
    }
    for byte in 2..=u8::MAX {
        assert!(bool::from_bytes(Cow::Borrowed(&[byte])).is_err());
    }
    for bytes in [&[][..], &[0, 1][..]] {
        assert!(bool::from_bytes(Cow::Borrowed(bytes)).is_err());
    }
}

#[test]
fn canonical_bounds_distinguish_empty_variable_and_exact_width_records() {
    for (bound, accepted) in [
        (Bound::Unbounded, [true, true, true, true]),
        (
            Bound::Bounded {
                max_size: 2,
                is_fixed_size: false,
            },
            [true, true, true, false],
        ),
        (
            Bound::Bounded {
                max_size: 2,
                is_fixed_size: true,
            },
            [false, false, true, false],
        ),
        (
            Bound::Bounded {
                max_size: 0,
                is_fixed_size: true,
            },
            [true, false, false, false],
        ),
    ] {
        for (len, accepted) in accepted.into_iter().enumerate() {
            assert_eq!(bound.check(&vec![0; len]).is_ok(), accepted);
        }
    }
}
