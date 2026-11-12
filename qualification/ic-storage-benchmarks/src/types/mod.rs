//! Concrete benchmark types; case metadata comes from the shared catalogue.
use serde::{Deserialize, Serialize};
mod breadth;
pub use breadth::{BoundedBytes, IdentityKey, OptionalU64, TupleKey};

/// Concrete collection and key/value types selected for one fresh canister.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Case {
    pub collection: crate::Collection,
    pub key: crate::DataType,
    pub value: crate::DataType,
}

/// Construct or match a concrete case using the authoritative type catalogue.
#[macro_export]
macro_rules! case {
    ($collection:ident, $key:ident, $value:ident) => {
        $crate::Case {
            collection: $crate::Collection::$collection,
            key: $crate::DataType::$key,
            value: $crate::DataType::$value,
        }
    };
}

pub const DEFAULT_CASES: &[Case] = &[
    crate::case!(Map, U64, Bytes),
    crate::case!(Map, U64, U64),
    crate::case!(Map, U64, U128),
    crate::case!(Map, Array32, Array32),
    crate::case!(Map, String, String),
    crate::case!(Map, U64, WireRecord),
    crate::case!(Set, U64, Unit),
    crate::case!(Set, String, Unit),
    crate::case!(Vector, U64, U64),
    crate::case!(Vector, U64, Chunk),
    crate::case!(Vector, U64, Array32),
    crate::case!(Vector, U64, WireRecord),
    crate::case!(Cell, U64, Bytes),
    crate::case!(Cell, U64, WireRecord),
    crate::case!(Region, U64, Bytes),
    crate::case!(Map, Signed64, Signed64),
    crate::case!(Map, U128, U128),
    crate::case!(Map, Array64, U128),
    crate::case!(Map, U64, Array31),
    crate::case!(Map, U64, Array32),
    crate::case!(Map, U64, Array33),
    crate::case!(Map, U64, Array256),
    crate::case!(Set, Signed64, Unit),
    crate::case!(Set, Array64, Unit),
    crate::case!(Vector, U64, Signed64),
    crate::case!(Vector, U64, U128),
    crate::case!(Vector, U64, F64),
    crate::case!(Cell, U64, U128),
    crate::case!(Cell, U64, Array32),
    crate::case!(Vector, U64, Session),
];

impl Case {
    pub fn structure(self) -> &'static str {
        self.collection.name()
    }
    pub fn rows(self, rows: u32) -> u32 {
        if self.collection == crate::Collection::Cell {
            1
        } else {
            rows.min(self.key.population_limit())
        }
    }
    pub fn measures_decode(self, operation: super::Operation) -> bool {
        self == crate::case!(Vector, U64, Session)
            && matches!(
                operation,
                super::Operation::Control
                    | super::Operation::Get
                    | super::Operation::WarmGet
                    | super::Operation::Decode
                    | super::Operation::MissAbove
            )
    }
    pub fn operations(self) -> Vec<super::Operation> {
        crate::catalogue::operations(self)
    }
    pub fn key_type(self) -> &'static str {
        self.key.name()
    }
    pub fn value_type(self) -> &'static str {
        self.value.name()
    }
    pub fn key_bound(self) -> &'static str {
        match self.collection {
            crate::Collection::Vector | crate::Collection::Cell => "index",
            _ => self.key.bound(),
        }
    }
    pub fn value_bound(self) -> &'static str {
        self.value.bound()
    }
    pub fn payloads(self) -> &'static [u32] {
        self.value.payloads()
    }
    pub fn accepts_payload(self, payload: u32) -> bool {
        self.payloads().contains(&payload)
            || matches!(self.value, crate::DataType::Bytes | crate::DataType::Chunk)
                && [
                    8, 44, 45, 63, 64, 65, 80, 108, 109, 132, 235, 236, 237, 256, 4075, 4076, 4077,
                    65_515, 65_516, 65_517, 65_536, 1_048_576,
                ]
                .contains(&payload)
    }
    pub fn expected_value(self, key: u64, payload: u32, replacement: bool) -> Vec<u8> {
        self.value.expected_value(key, payload, replacement)
    }
}

///
/// BenchKey
///
/// Order-preserving mapping from logical trace IDs to concrete map keys.
///
pub trait BenchKey: Ord + Clone {
    fn from_id(id: u64) -> Self;
    fn id(&self) -> u64;
}
impl BenchKey for u64 {
    fn from_id(id: u64) -> Self {
        id
    }
    fn id(&self) -> u64 {
        *self
    }
}
///
/// Signed64
///
/// Signed integer with the same fixed-width codec on both backends; upstream
/// stable-structures does not implement Storable for primitive i64.
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Signed64(i64);
impl BenchKey for Signed64 {
    fn from_id(id: u64) -> Self {
        Self(i64::from_id(id))
    }
    fn id(&self) -> u64 {
        self.0.id()
    }
}
impl BenchValue for Signed64 {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        Self(i64::make(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.0.canonical()
    }
}
impl BenchKey for i64 {
    fn from_id(id: u64) -> Self {
        i64::try_from(id).expect("bounded benchmark key") - 32
    }
    fn id(&self) -> u64 {
        u64::try_from(*self + 32).unwrap()
    }
}
impl BenchKey for u128 {
    fn from_id(id: u64) -> Self {
        (u128::from(id) << 64) | 0xa5a5_0123_4567_89ab
    }
    fn id(&self) -> u64 {
        let id = (self >> 64) as u64;
        assert_eq!(*self, Self::from_id(id));
        id
    }
}

impl BenchKey for String {
    fn from_id(id: u64) -> Self {
        format!("account/{id:020}")
    }
    fn id(&self) -> u64 {
        let id = self.strip_prefix("account/").unwrap().parse().unwrap();
        assert_eq!(*self, Self::from_id(id));
        id
    }
}

///
/// BenchValue
///
/// Generate concrete values before timing and normalize results after timing.
///
pub trait BenchValue: Clone {
    fn make(key: u64, payload: u32, replacement: bool) -> Self;
    fn canonical(self) -> Vec<u8>;
}
impl BenchValue for Vec<u8> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        super::value(key, payload, replacement)
    }
    fn canonical(self) -> Vec<u8> {
        self
    }
}
impl BenchValue for u64 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        key ^ 0xa5a5_0000_0000_0000 ^ if replacement { 1 << 63 } else { 0 }
    }
    fn canonical(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl BenchValue for i64 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        i64::from_le_bytes(u64::make(key, 8, replacement).to_le_bytes())
    }
    fn canonical(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl BenchValue for f64 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        // Include signed zero, subnormals, infinities and a NaN payload.
        let bits = [
            0,
            1 << 63,
            1,
            (1 << 63) | 1,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
            0x7ff8_0000_0000_0042,
            0x3ff8_0000_0000_0000,
        ][key as usize % 8];
        Self::from_bits(bits ^ if replacement { 1 << 63 } else { 0 })
    }
    fn canonical(self) -> Vec<u8> {
        self.to_bits().to_le_bytes().to_vec()
    }
}
impl BenchValue for u128 {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        (u128::from(key) << 64) | u128::from(u64::make(key, 8, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl<const N: usize> BenchValue for [u8; N] {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        super::value(key, u32::try_from(N).unwrap(), replacement)
            .try_into()
            .unwrap()
    }
    fn canonical(self) -> Vec<u8> {
        self.to_vec()
    }
}
impl BenchValue for String {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        // Include non-ASCII UTF-8 while preserving the declared byte size.
        let letter =
            char::from_u32(0xe0 + (key % 16) as u32 + u32::from(replacement) * 16).unwrap();
        letter.to_string().repeat(payload as usize / 2)
    }
    fn canonical(self) -> Vec<u8> {
        self.into_bytes()
    }
}

///
/// ChunkBytes
///
/// Bounded raw chunk bytes, matching Canic's 1 MiB variable-length vector contract.
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ChunkBytes(Vec<u8>);
impl BenchValue for ChunkBytes {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        assert!(payload <= 1_048_576);
        Self(super::value(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.0
    }
}

///
/// WireRecord
///
/// Application-shaped record with the same bounded IC Wire codec on both stacks.
///

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WireRecord {
    account: u64,
    balance: u128,
    generation: u32,
    tag: [u8; 32],
}
impl WireRecord {
    pub fn encode(&self) -> Result<Vec<u8>, ic_wire::Error> {
        ic_wire::native::encode(self, RECORD_LIMITS)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ic_wire::Error> {
        ic_wire::native::decode(bytes, RECORD_LIMITS)
    }
}
const RECORD_LIMITS: ic_wire::Limits = ic_wire::Limits {
    bytes: 256,
    allocation: 16 * 1024,
    nodes: 256,
    depth: 8,
};
impl BenchValue for WireRecord {
    fn make(key: u64, _: u32, replacement: bool) -> Self {
        Self {
            account: key,
            balance: u128::make(key, 16, replacement),
            generation: u32::from(replacement),
            tag: <[u8; 32]>::make(key, 32, replacement),
        }
    }
    fn canonical(self) -> Vec<u8> {
        // Independent of the storage codec: validate every decoded field.
        let mut bytes = self.account.to_le_bytes().to_vec();
        bytes.extend(self.balance.to_le_bytes());
        bytes.extend(self.generation.to_le_bytes());
        bytes.extend(self.tag);
        bytes
    }
}

#[cfg(target_arch = "wasm32")]
mod codec {
    use super::{ChunkBytes, SessionEnvelope, Signed64, WireRecord, measured_session_decode};
    #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
    use crate::upstream::{Storable, storable::Bound};
    #[cfg(feature = "owned")]
    use ic_storage::adapter::{Bound, CodecError, Storable};
    use std::borrow::Cow;
    impl Storable for ChunkBytes {
        const BOUND: Bound = Bound::Bounded {
            max_size: 1_048_576,
            is_fixed_size: false,
        };
        #[cfg(feature = "owned")]
        fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
            Self::check_bounds(&self.0)?;
            Ok(Cow::Borrowed(&self.0))
        }
        #[cfg(feature = "owned")]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
            Self::check_bounds(&bytes)?;
            Ok(Self(bytes.into_owned()))
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn to_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Borrowed(&self.0)
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn into_bytes(self) -> Vec<u8> {
            self.0
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            Self(bytes.into_owned())
        }
    }
    impl Storable for SessionEnvelope {
        const BOUND: Bound = Bound::Bounded {
            max_size: 4096,
            is_fixed_size: false,
        };
        #[cfg(feature = "owned")]
        fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
            self.encode()
                .map(Cow::Owned)
                .map_err(|_| CodecError::Invalid("session envelope"))
        }
        #[cfg(feature = "owned")]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
            measured_session_decode(&bytes).map_err(|_| CodecError::Invalid("session envelope"))
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn to_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Owned(self.encode().unwrap())
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn into_bytes(self) -> Vec<u8> {
            self.encode().unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            measured_session_decode(&bytes).unwrap()
        }
    }
    impl Storable for Signed64 {
        const BOUND: Bound = Bound::Bounded {
            max_size: 8,
            is_fixed_size: true,
        };
        #[cfg(feature = "owned")]
        fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
            Ok(Cow::Owned(self.0.to_le_bytes().to_vec()))
        }
        #[cfg(feature = "owned")]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
            Ok(Self(i64::from_le_bytes(
                bytes
                    .as_ref()
                    .try_into()
                    .map_err(|_| CodecError::Invalid("benchmark signed width"))?,
            )))
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn to_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Owned(self.0.to_le_bytes().to_vec())
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn into_bytes(self) -> Vec<u8> {
            self.0.to_le_bytes().to_vec()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            Self(i64::from_le_bytes(bytes.as_ref().try_into().unwrap()))
        }
    }
    impl Storable for WireRecord {
        const BOUND: Bound = Bound::Bounded {
            max_size: 256,
            is_fixed_size: false,
        };
        #[cfg(feature = "owned")]
        fn to_bytes(&self) -> Result<Cow<'_, [u8]>, CodecError> {
            self.encode()
                .map(Cow::Owned)
                .map_err(|_| CodecError::Invalid("benchmark Wire record"))
        }
        #[cfg(feature = "owned")]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, CodecError> {
            Self::decode(&bytes).map_err(|_| CodecError::Invalid("benchmark Wire record"))
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn to_bytes(&self) -> Cow<'_, [u8]> {
            Cow::Owned(self.encode().unwrap())
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn into_bytes(self) -> Vec<u8> {
            self.encode().unwrap()
        }
        #[cfg(any(feature = "dfinity", feature = "dfinity-head"))]
        fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
            Self::decode(&bytes).unwrap()
        }
    }
}

#[cfg(test)]
mod tests;

///
/// SessionEnvelope
///
/// Canic-shaped outer storage record; inner Auth bytes remain opaque synthetic data.
///

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct SessionEnvelope {
    #[serde(with = "serde_bytes")]
    session: Vec<u8>,
}
impl SessionEnvelope {
    pub fn encode(&self) -> Result<Vec<u8>, ic_wire::Error> {
        ic_wire::native::encode(&(*b"CSW\x01", self), SESSION_LIMITS)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, ic_wire::Error> {
        if !bytes.starts_with(b"CSW\x01") {
            return Err(ic_wire::Error::Identity);
        }
        ic_wire::native::decode::<([u8; 4], Self)>(bytes, SESSION_LIMITS).map(|(_, value)| value)
    }
}
const SESSION_LIMITS: ic_wire::Limits = ic_wire::Limits {
    bytes: 64 * 1024 * 1024,
    allocation: 1024 * 1024 * 1024,
    nodes: 64 * 1024 * 1024,
    depth: 128,
};
impl BenchValue for SessionEnvelope {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        let mut session = crate::value(key, payload, replacement);
        session[..8].copy_from_slice(&key.to_le_bytes());
        Self { session }
    }
    fn canonical(self) -> Vec<u8> {
        self.session
    }
}
#[cfg(target_arch = "wasm32")]
thread_local! { static SESSION_DECODE: std::cell::Cell<crate::DecodeMetrics> = const { std::cell::Cell::new(crate::DecodeMetrics { calls:0,bytes:0,instructions:0 }) }; }
#[cfg(target_arch = "wasm32")]
pub fn reset_decode_metrics() {
    SESSION_DECODE.with(|m| m.set(crate::DecodeMetrics::default()));
}
#[cfg(target_arch = "wasm32")]
pub fn decode_metrics() -> crate::DecodeMetrics {
    SESSION_DECODE.with(std::cell::Cell::get)
}
#[cfg(target_arch = "wasm32")]
fn measured_session_decode(bytes: &[u8]) -> Result<SessionEnvelope, ic_wire::Error> {
    let start = ic_platform_runtime::ic0::performance_counter(0);
    let decoded = SessionEnvelope::decode(bytes);
    let cost = ic_platform_runtime::ic0::performance_counter(0) - start;
    SESSION_DECODE.with(|m| {
        let mut metrics = m.get();
        metrics.calls += 1;
        metrics.bytes += bytes.len() as u64;
        metrics.instructions += cost;
        m.set(metrics);
    });
    decoded
}

impl BenchKey for WireRecord {
    fn from_id(id: u64) -> Self {
        Self::make(id, 60, false)
    }
    fn id(&self) -> u64 {
        assert_eq!(*self, Self::from_id(self.account));
        self.account
    }
}
impl BenchKey for SessionEnvelope {
    fn from_id(id: u64) -> Self {
        let mut session = id.to_be_bytes().to_vec();
        session.resize(32, 0x5a);
        Self { session }
    }
    fn id(&self) -> u64 {
        let id = u64::from_be_bytes(self.session[..8].try_into().unwrap());
        assert_eq!(*self, Self::from_id(id));
        id
    }
}
impl BenchKey for ChunkBytes {
    fn from_id(id: u64) -> Self {
        Self(Vec::<u8>::from_id(id))
    }
    fn id(&self) -> u64 {
        self.0.id()
    }
}
