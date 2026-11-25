//! Storage-owned record codec contract. Application records normally use IC Wire.
use std::borrow::Cow;

///
/// Bound
///
/// Maximum encoded record size, with an optional exact-width requirement.
///
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    /// No type-level bound; collection resource limits still apply.
    Unbounded,
    /// A finite encoded-size limit.
    Bounded {
        /// Maximum encoded byte count.
        max_size: u32,
        /// Whether every encoding must have exactly the maximum length.
        is_fixed_size: bool,
    },
}

impl Bound {
    pub(crate) const fn check(self, bytes: &[u8]) {
        if let Self::Bounded {
            max_size,
            is_fixed_size,
        } = self
            && !(bytes.len() <= max_size as usize
                && (!is_fixed_size || bytes.len() == max_size as usize))
        {
            panic!("record violates codec bound");
        }
    }
}

///
/// Storable
///
/// Encode and decode a collection record. The caller owns its schema; Storage
/// enforces its declared byte bound before writes and after reading bytes.
/// Collection checks are independent of overrides of the convenience helpers.
///
pub trait Storable: Sized {
    /// Record size contract.
    const BOUND: Bound;
    /// Borrow or allocate an encoding.
    ///
    fn to_bytes(&self) -> Cow<'_, [u8]>;
    /// Consume a value into its encoding.
    ///
    fn into_bytes(self) -> Vec<u8> {
        self.to_bytes().into_owned()
    }
    /// Decode application bytes, trapping on invalid encodings.
    ///
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self;
    /// Enforce the declared maximum and exact width where applicable.
    ///
    fn check_bounds(bytes: &[u8]) {
        Self::BOUND.check(bytes)
    }
    /// Encode and enforce the declared bound.
    ///
    fn to_bytes_checked(&self) -> Cow<'_, [u8]> {
        let bytes = self.to_bytes();
        Self::BOUND.check(&bytes);
        bytes
    }
    /// Consume, encode and enforce the declared bound.
    ///
    fn into_bytes_checked(self) -> Vec<u8> {
        let bytes = self.into_bytes();
        Self::BOUND.check(&bytes);
        bytes
    }
}

macro_rules! integer {
    ($($ty:ty),* $(,)?) => {$ (
        impl Storable for $ty {
            const BOUND: Bound = Bound::Bounded { max_size: size_of::<Self>() as u32, is_fixed_size: true };
            fn to_bytes(&self) -> Cow<'_, [u8]> { Cow::Owned(self.to_le_bytes().to_vec()) }
            fn from_bytes(bytes: Cow<'_, [u8]>) -> Self { Self::from_le_bytes(bytes.as_ref().try_into().expect("integer width")) }
        }
    )*};
}
integer!(u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);
impl Storable for u8 {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(std::slice::from_ref(self))
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self::from_le_bytes(bytes.as_ref().try_into().expect("integer width"))
    }
}
impl Storable for () {
    const BOUND: Bound = Bound::Bounded {
        max_size: 0,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(&[])
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self::check_bounds(&bytes)
    }
}
impl Storable for bool {
    const BOUND: Bound = Bound::Bounded {
        max_size: 1,
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(if *self { &[1] } else { &[0] })
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        match bytes.as_ref() {
            [0] => false,
            [1] => true,
            _ => panic!("invalid boolean encoding"),
        }
    }
}
impl Storable for Vec<u8> {
    const BOUND: Bound = Bound::Unbounded;
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self)
    }
    fn into_bytes(self) -> Vec<u8> {
        self
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        bytes.into_owned()
    }
}
impl Storable for String {
    const BOUND: Bound = Bound::Unbounded;
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self.as_bytes())
    }
    fn into_bytes(self) -> Vec<u8> {
        self.into_bytes()
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        Self::from_utf8(bytes.into_owned()).expect("invalid UTF-8 record")
    }
}
impl<const N: usize> Storable for [u8; N] {
    const BOUND: Bound = Bound::Bounded {
        max_size: {
            assert!(N <= u32::MAX as usize);
            N as u32
        },
        is_fixed_size: true,
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        Cow::Borrowed(self)
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        bytes.as_ref().try_into().expect("invalid byte array width")
    }
}
impl<T: Storable> Storable for Option<T> {
    const BOUND: Bound = match T::BOUND {
        Bound::Unbounded => Bound::Unbounded,
        Bound::Bounded { max_size, .. } => Bound::Bounded {
            max_size: max_size.checked_add(1).expect("optional bound overflow"),
            is_fixed_size: false,
        },
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut bytes = vec![u8::from(self.is_some())];
        if let Some(value) = self {
            bytes.extend_from_slice(&value.to_bytes_checked());
        }
        Cow::Owned(bytes)
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        match bytes.as_ref().split_first() {
            Some((0, [])) => None,
            Some((1, payload)) => {
                T::check_bounds(payload);
                Some(T::from_bytes(Cow::Borrowed(payload)))
            }
            _ => panic!("invalid optional record tag"),
        }
    }
}

const fn pair_bound(left: Bound, right: Bound) -> Bound {
    match (left, right) {
        (
            Bound::Bounded {
                max_size: a,
                is_fixed_size: af,
            },
            Bound::Bounded {
                max_size: b,
                is_fixed_size: bf,
            },
        ) => {
            let width = a as u64 + b as u64 + 4;
            if width <= u32::MAX as u64 {
                Bound::Bounded {
                    max_size: width as u32,
                    is_fixed_size: af && bf,
                }
            } else {
                Bound::Unbounded
            }
        }
        _ => Bound::Unbounded,
    }
}

// All tuple components except the last carry a four-byte length.
fn push_component(bytes: &mut Vec<u8>, component: &[u8]) {
    let length = u32::try_from(component.len()).expect("tuple component exceeds u32");
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(component);
}

fn take_component<T: Storable>(bytes: &mut &[u8]) -> T {
    let (prefix, payload) = bytes.split_at(4);
    let length = u32::from_le_bytes(prefix.try_into().unwrap()) as usize;
    let (component, rest) = payload.split_at(length);
    *bytes = rest;
    decode_component(component)
}

fn decode_component<T: Storable>(bytes: &[u8]) -> T {
    T::BOUND.check(bytes);
    T::from_bytes(Cow::Borrowed(bytes))
}

impl<A: Storable, B: Storable> Storable for (A, B) {
    const BOUND: Bound = pair_bound(A::BOUND, B::BOUND);

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let left = self.0.to_bytes();
        let right = self.1.to_bytes();
        A::BOUND.check(&left);
        B::BOUND.check(&right);
        let mut bytes = Vec::with_capacity(4 + left.len() + right.len());
        push_component(&mut bytes, &left);
        bytes.extend_from_slice(&right);
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let mut payload = bytes.as_ref();
        (take_component(&mut payload), decode_component(payload))
    }
}

impl<A: Storable, B: Storable, C: Storable> Storable for (A, B, C) {
    const BOUND: Bound = pair_bound(A::BOUND, pair_bound(B::BOUND, C::BOUND));

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let a = self.0.to_bytes();
        let b = self.1.to_bytes();
        let c = self.2.to_bytes();
        A::BOUND.check(&a);
        B::BOUND.check(&b);
        C::BOUND.check(&c);
        let mut bytes = Vec::with_capacity(8 + a.len() + b.len() + c.len());
        push_component(&mut bytes, &a);
        push_component(&mut bytes, &b);
        bytes.extend_from_slice(&c);
        Cow::Owned(bytes)
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let mut payload = bytes.as_ref();
        (
            take_component(&mut payload),
            take_component(&mut payload),
            decode_component(payload),
        )
    }
}
