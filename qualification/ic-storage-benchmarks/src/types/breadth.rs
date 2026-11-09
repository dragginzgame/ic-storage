//! Type geometry controls informed by ICSS's B-tree grid and Vec/Storable tests.
use super::{BenchKey, BenchValue};

///
/// BoundedBytes
///
/// Variable bytes with an explicit maximum, independent of their actual length.
/// Equivalent to ICSS's bounded Blob geometry; both backends use this same codec.
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BoundedBytes<const MAX: u32>(Vec<u8>);
impl<const MAX: u32> BenchValue for BoundedBytes<MAX> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        assert!(payload <= MAX);
        Self(crate::value(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.0
    }
}
impl<const MAX: u32> BenchKey for BoundedBytes<MAX> {
    fn from_id(id: u64) -> Self {
        assert!(MAX >= 8);
        let mut bytes = id.to_be_bytes().to_vec();
        // The identity prefix preserves order while the suffix varies key size.
        bytes.resize(8 + (id % u64::from(MAX - 7)) as usize, 0x5a);
        Self(bytes)
    }
    fn id(&self) -> u64 {
        let id = u64::from_be_bytes(self.0[..8].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
impl BenchKey for Vec<u8> {
    fn from_id(id: u64) -> Self {
        let mut bytes = id.to_be_bytes().to_vec();
        // Exercise node overflow even at the bounded 64-row type preset.
        bytes.resize(8 + (id.wrapping_mul(17) % 1025) as usize, 0x5a);
        bytes
    }
    fn id(&self) -> u64 {
        let id = u64::from_be_bytes(self[..8].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
impl BenchKey for u32 {
    fn from_id(id: u64) -> Self {
        Self::try_from(id).expect("benchmark key exceeds u32")
    }
    fn id(&self) -> u64 {
        u64::from(*self)
    }
}

///
/// TupleKey
///
/// Application-owned composite key inspired by ICSS's (u64, [u8;20]) tests.
/// Uses a shared fixed 28-byte codec; does not claim native tuple-codec parity.
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TupleKey(u64, [u8; 20]);
impl BenchKey for TupleKey {
    fn from_id(id: u64) -> Self {
        let mut tail = [0x5a; 20];
        tail[12..].copy_from_slice(&(id & 255).to_be_bytes());
        Self(id >> 8, tail)
    }
    fn id(&self) -> u64 {
        let id = (self.0 << 8) | u64::from_be_bytes(self.1[12..].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}

///
/// OptionalU64
///
/// Application codec for alternating None/Some values, with a variable nine-byte
/// bound. ICSS 0.7.2's native Option<u64> incorrectly declares a fixed bound;
/// its checked encoding of None traps. This explicitly shared codec avoids
/// presenting that unsupported combination as a successful paired benchmark.
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OptionalU64(Option<u64>);
impl BenchValue for OptionalU64 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        Self(((key & 1 != 0) ^ replacement).then(|| u64::make(key, 8, replacement)))
    }
    fn canonical(self) -> Vec<u8> {
        // Fixed normalized output retains the variant and the complete payload.
        let mut bytes = vec![u8::from(self.0.is_some())];
        bytes.extend(self.0.unwrap_or(0).to_le_bytes());
        bytes
    }
}

impl BenchValue for () {
    fn make(_: u64, _: u32, _: bool) -> Self {}
    fn canonical(self) -> Vec<u8> {
        vec![]
    }
}
macro_rules! unsigned_values {
    ($($ty:ty),*) => {$ (
        impl BenchValue for $ty {
            fn make(key: u64, _: u32, replacement: bool) -> Self {
                (key as Self) ^ (0xa5a5_a5a5u32 as Self)
                    ^ if replacement { 1 << (Self::BITS - 1) } else { 0 }
            }
            fn canonical(self) -> Vec<u8> { self.to_le_bytes().to_vec() }
        }
    )*};
}
unsigned_values!(u8, u16, u32);
impl BenchValue for bool {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        (key & 1 != 0) ^ replacement
    }
    fn canonical(self) -> Vec<u8> {
        vec![u8::from(self)]
    }
}
impl BenchValue for f32 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        let bits = [
            0,
            1 << 31,
            1,
            (1 << 31) | 1,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0042,
            0x3fc0_0000,
        ][key as usize % 8];
        Self::from_bits(bits ^ if replacement { 1 << 31 } else { 0 })
    }
    fn canonical(self) -> Vec<u8> {
        self.to_bits().to_le_bytes().to_vec()
    }
}

#[cfg(any(
    test,
    feature = "owned",
    feature = "dfinity",
    all(target_arch = "wasm32", feature = "dfinity-head")
))]
mod codec {
    use super::*;
    use std::borrow::Cow;
    // Define each application codec once, then project it to the two existing
    // Storable contracts. This helper stays in the unpublished comparison crate.
    pub(super) trait Encoding: Sized {
        const MAX: u32;
        const FIXED: bool;
        fn encode(&self) -> Cow<'_, [u8]>;
        fn decode(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str>;
        fn into_encoded(self) -> Vec<u8> {
            self.encode().into_owned()
        }
    }
    impl<const MAX: u32> Encoding for BoundedBytes<MAX> {
        const MAX: u32 = MAX;
        const FIXED: bool = false;
        fn encode(&self) -> Cow<'_, [u8]> {
            Cow::Borrowed(&self.0)
        }
        fn decode(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
            if bytes.len() > MAX as usize {
                return Err("bounded bytes exceed maximum");
            }
            Ok(Self(bytes.into_owned()))
        }
        fn into_encoded(self) -> Vec<u8> {
            self.0
        }
    }
    impl Encoding for TupleKey {
        const MAX: u32 = 28;
        const FIXED: bool = true;
        fn encode(&self) -> Cow<'_, [u8]> {
            let mut bytes = self.0.to_le_bytes().to_vec();
            bytes.extend(self.1);
            Cow::Owned(bytes)
        }
        fn decode(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
            if bytes.len() != 28 {
                return Err("tuple key width");
            }
            Ok(Self(
                u64::from_le_bytes(bytes[..8].try_into().unwrap()),
                bytes[8..].try_into().unwrap(),
            ))
        }
    }
    impl Encoding for OptionalU64 {
        const MAX: u32 = 9;
        const FIXED: bool = false;
        fn encode(&self) -> Cow<'_, [u8]> {
            let mut bytes = vec![u8::from(self.0.is_some())];
            if let Some(value) = self.0 {
                bytes.extend(value.to_le_bytes());
            }
            Cow::Owned(bytes)
        }
        fn decode(bytes: Cow<'_, [u8]>) -> Result<Self, &'static str> {
            match bytes.as_ref() {
                [0] => Ok(Self(None)),
                [1, payload @ ..] if payload.len() == 8 => {
                    Ok(Self(Some(u64::from_le_bytes(payload.try_into().unwrap()))))
                }
                _ => Err("optional u64 encoding"),
            }
        }
    }
    macro_rules! codecs {
    (($($generics:tt)*), $ty:ty) => {
        #[cfg(feature = "owned")]
        impl $($generics)* ic_storage::storable::Storable for $ty {
            const BOUND: ic_storage::storable::Bound = ic_storage::storable::Bound::Bounded {
                max_size: <Self as Encoding>::MAX, is_fixed_size: <Self as Encoding>::FIXED,
            };
            fn to_bytes(&self) -> Result<Cow<'_, [u8]>, ic_storage::storable::CodecError> { Ok(self.encode()) }
            fn into_bytes(self) -> Result<Vec<u8>, ic_storage::storable::CodecError> { Ok(self.into_encoded()) }
            fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, ic_storage::storable::CodecError> {
                Self::decode(bytes).map_err(ic_storage::storable::CodecError::Invalid)
            }
        }
        #[cfg(any(feature = "dfinity", all(target_arch = "wasm32", feature = "dfinity-head")))]
        impl $($generics)* crate::upstream::Storable for $ty {
            const BOUND: crate::upstream::storable::Bound = crate::upstream::storable::Bound::Bounded {
                max_size: <Self as Encoding>::MAX, is_fixed_size: <Self as Encoding>::FIXED,
            };
            fn to_bytes(&self) -> Cow<'_, [u8]> { self.encode() }
            fn into_bytes(self) -> Vec<u8> { self.into_encoded() }
            fn from_bytes(bytes: Cow<'_, [u8]>) -> Self { Self::decode(bytes).expect("benchmark application codec") }
        }
    };
}
    codecs!((<const MAX: u32>), BoundedBytes<MAX>);
    codecs!((), TupleKey);
    codecs!((), OptionalU64);
}

#[cfg(test)]
mod tests;

impl BenchKey for OptionalU64 {
    fn from_id(id: u64) -> Self {
        Self(id.checked_sub(1))
    }
    fn id(&self) -> u64 {
        self.0.map_or(0, |v| v.checked_add(1).unwrap())
    }
}

impl BenchValue for TupleKey {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        Self(key, <[u8; 20]>::make(key, 20, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        let mut bytes = self.0.to_le_bytes().to_vec();
        bytes.extend(self.1);
        bytes
    }
}
