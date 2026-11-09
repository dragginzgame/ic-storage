//! Independent typed trace construction and field-wise result normalization.
use crate::{BenchKey, BenchValue, BenchmarkPrincipal, codec_reference::storable::Blob};

impl<const N: usize> BenchKey for [u8; N] {
    fn from_id(id: u64) -> Self {
        assert!(N >= 8);
        let mut bytes = [0x5a; N];
        let start = if N == 32 { 0 } else { N - 8 };
        bytes[start..start + 8].copy_from_slice(&id.to_be_bytes());
        bytes
    }
    fn id(&self) -> u64 {
        let start = if N == 32 { 0 } else { N - 8 };
        let id = u64::from_be_bytes(self[start..start + 8].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
impl BenchKey for () {
    fn from_id(id: u64) -> Self {
        assert_eq!(id, 0);
    }
    fn id(&self) -> u64 {
        0
    }
}
impl BenchKey for bool {
    fn from_id(id: u64) -> Self {
        assert!(id < 2);
        id != 0
    }
    fn id(&self) -> u64 {
        u64::from(*self)
    }
}
macro_rules! narrow_keys {
    ($($ty:ty),*) => {$ (
        impl BenchKey for $ty {
            fn from_id(id:u64)->Self {Self::try_from(id).expect("finite benchmark key domain")}
            fn id(&self)->u64 {u64::from(*self)}
        }
    )*};
}
narrow_keys!(u8, u16);
impl<const N: usize> BenchValue for Blob<N> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        let bytes = crate::value(key, payload.min(N as u32), replacement);
        Self::try_from(bytes.as_slice()).unwrap()
    }
    fn canonical(self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}
impl<const N: usize> BenchKey for Blob<N> {
    fn from_id(id: u64) -> Self {
        assert!(N >= 8);
        let mut bytes = vec![0x5a; N];
        bytes[N - 8..].copy_from_slice(&id.to_be_bytes());
        Self::try_from(bytes.as_slice()).unwrap()
    }
    fn id(&self) -> u64 {
        let id = u64::from_be_bytes(self.as_slice()[N - 8..].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
impl BenchValue for BenchmarkPrincipal {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        assert!(payload <= 29);
        let mut bytes = crate::value(key, payload, replacement);
        if let Some(tag) = bytes.last_mut() {
            *tag = [1, 2, 3, 4, 7][(key as usize + usize::from(replacement)) % 5];
        }
        Self::from_slice(&bytes)
    }
    fn canonical(self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}
impl BenchKey for BenchmarkPrincipal {
    fn from_id(id: u64) -> Self {
        let mut bytes = vec![0x5a; 29];
        bytes[..8].copy_from_slice(&id.to_be_bytes());
        bytes[28] = 1;
        Self::from_slice(&bytes)
    }
    fn id(&self) -> u64 {
        let id = u64::from_be_bytes(self.as_slice()[..8].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
// Component lengths make normalization unambiguous without using the storage
// tuple codec. Every decoded field contributes to the independently checked result.
fn components(parts: impl IntoIterator<Item = Vec<u8>>) -> Vec<u8> {
    let mut result = Vec::new();
    for part in parts {
        result.extend(u32::try_from(part.len()).unwrap().to_le_bytes());
        result.extend(part);
    }
    result
}
impl<A: BenchValue, B: BenchValue> BenchValue for (A, B) {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        (
            A::make(key, payload, replacement),
            B::make(key.rotate_left(13), payload, replacement),
        )
    }
    fn canonical(self) -> Vec<u8> {
        components([self.0.canonical(), self.1.canonical()])
    }
}
impl<A: BenchValue, B: BenchValue, C: BenchValue> BenchValue for (A, B, C) {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        (
            A::make(key, payload, replacement),
            B::make(key.rotate_left(13), payload, replacement),
            C::make(key.rotate_left(29), payload, replacement),
        )
    }
    fn canonical(self) -> Vec<u8> {
        components([self.0.canonical(), self.1.canonical(), self.2.canonical()])
    }
}
impl<A: BenchKey, B: BenchKey> BenchKey for (A, B) {
    fn from_id(id: u64) -> Self {
        (A::from_id(id >> 8), B::from_id(id & 255))
    }
    fn id(&self) -> u64 {
        let id = (self.0.id() << 8) | self.1.id();
        assert!(*self == Self::from_id(id));
        id
    }
}
impl<A: BenchKey, B: BenchKey, C: BenchKey> BenchKey for (A, B, C) {
    fn from_id(id: u64) -> Self {
        (
            A::from_id(id >> 16),
            B::from_id((id >> 8) & 255),
            C::from_id(id & 255),
        )
    }
    fn id(&self) -> u64 {
        let id = (self.0.id() << 16) | (self.1.id() << 8) | self.2.id();
        assert!(*self == Self::from_id(id));
        id
    }
}
impl<T: BenchValue> BenchValue for Option<T> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        ((key & 1 != 0) ^ replacement).then(|| T::make(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        match self {
            None => vec![0],
            Some(value) => {
                let mut bytes = vec![1];
                bytes.extend(value.canonical());
                bytes
            }
        }
    }
}
impl<T: BenchKey> BenchKey for Option<T> {
    fn from_id(id: u64) -> Self {
        id.checked_sub(1).map(T::from_id)
    }
    fn id(&self) -> u64 {
        self.as_ref()
            .map_or(0, |value| value.id().checked_add(1).unwrap())
    }
}

/// ICSS's Vec test control: an eight-byte scalar with a loose variable bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct UnfixedU64<const MAX: u32>(u64);
impl<const MAX: u32> BenchValue for UnfixedU64<MAX> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        Self(u64::make(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.0.canonical()
    }
}
impl<const MAX: u32> BenchKey for UnfixedU64<MAX> {
    fn from_id(id: u64) -> Self {
        Self(id)
    }
    fn id(&self) -> u64 {
        self.0
    }
}
impl<const MAX: u32> crate::codec_reference::Storable for UnfixedU64<MAX> {
    const BOUND: crate::codec_reference::storable::Bound =
        crate::codec_reference::storable::Bound::Bounded {
            max_size: MAX,
            is_fixed_size: false,
        };
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        crate::codec_reference::Storable::to_bytes(&self.0)
    }
    fn into_bytes(self) -> Vec<u8> {
        crate::codec_reference::Storable::into_bytes(self.0)
    }
    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        Self(crate::codec_reference::Storable::from_bytes(bytes))
    }
}
