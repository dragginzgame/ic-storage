//! One finite catalogue owns the ICSS test types and Storage's extra types.
use crate::{BenchValue, Case, Operation};
mod models;
pub use models::UnfixedU64;
use serde::{Deserialize, Serialize};

/// Collection form selected by the complete type matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(clap::ValueEnum))]
pub enum Collection {
    Map,
    Set,
    Vector,
    Cell,
    Region,
}
impl Collection {
    pub const ALL: [Self; 5] = [Self::Map, Self::Set, Self::Vector, Self::Cell, Self::Region];
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Map => "Map",
            Self::Set => "Set",
            Self::Vector => "Vector",
            Self::Cell => "Cell",
            Self::Region => "Region",
        }
    }
}

// Callback expansion gives metadata, generation and Wasm monomorphization the
// same roster. The type expression is the actual concrete Rust type measured.
#[macro_export]
macro_rules! type_catalogue {
    ($callback:ident, $context:tt) => { $callback! { $context;
        U8 => (u8, "u8", "both", true, "fixed", &[1], false);
        U16 => (u16, "u16", "both", true, "fixed", &[2], false);
        U32 => (u32, "u32", "both", true, "fixed", &[4], false);
        U64 => (u64, "u64", "both", true, "fixed", &[8], false);
        U128 => (u128, "u128", "both", true, "fixed", &[16], false);
        F32 => (f32, "f32", "both", false, "fixed", &[4], false);
        F64 => (f64, "f64", "both", false, "fixed", &[8], false);
        Bool => (bool, "bool", "both", true, "fixed", &[1], false);
        Unit => ((), "()", "both", true, "fixed", &[0], false);
        Bytes => (Vec<u8>, "Vec<u8>", "both", true, "unbounded", &[0,31,32,33,256,1024,4096], false);
        String => (String, "String", "both", true, "unbounded", &[0,32,1024,4096], false);
        Array20 => ([u8;20], "[u8;20]", "both", true, "fixed", &[20], false);
        Array31 => ([u8;31], "[u8;31]", "storage", true, "fixed", &[31], false);
        Array32 => ([u8;32], "[u8;32]", "both", true, "fixed", &[32], false);
        Array33 => ([u8;33], "[u8;33]", "storage", true, "fixed", &[33], false);
        Array64 => ([u8;64], "[u8;64]", "storage", true, "fixed", &[64], false);
        Array256 => ([u8;256], "[u8;256]", "storage", true, "fixed", &[256], false);
        Blob8 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<8>>, "ICSS Blob<8>", "icss", true, "bounded-variable", &[0,8], false);
        Blob10 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<10>>, "ICSS Blob<10>", "icss", true, "bounded-variable", &[0,8,10], false);
        Blob20 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<20>>, "ICSS Blob<20>", "icss", true, "bounded-variable", &[0,8,20], false);
        Blob40 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<40>>, "ICSS Blob<40>", "icss", true, "bounded-variable", &[0,8,40], false);
        Blob48 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<48>>, "ICSS Blob<48>", "icss", true, "bounded-variable", &[0,8,48], false);
        Blob80 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<80>>, "ICSS Blob<80>", "icss", true, "bounded-variable", &[0,8,80], false);
        Blob300 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<300>>, "ICSS Blob<300>", "icss", true, "bounded-variable", &[0,8,300], false);
        Blob65536 => ($crate::ReferenceValue<$crate::codec_reference::storable::Blob<65536>>, "ICSS Blob<65536>", "icss", true, "bounded-variable", &[0,8,256,65536], false);
        PairFixed => ($crate::ReferenceValue<(u64,[u8;20])>, "(u64,[u8;20])", "icss", true, "fixed", &[28], false);
        TripleFixed => ($crate::ReferenceValue<(u64,[u8;20],[u8;20])>, "(u64,[u8;20],[u8;20])", "icss", true, "fixed", &[48], false);
        Pair48 => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<48>)>, "(u64,Blob<48>)", "icss", true, "bounded-variable", &[0,8], false);
        Pair300 => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<300>)>, "(u64,Blob<300>)", "icss", true, "bounded-variable", &[0,8], false);
        Triple40_80 => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<40>,$crate::codec_reference::storable::Blob<80>)>, "(u64,Blob<40>,Blob<80>)", "icss", true, "bounded-variable", &[0,8], false);
        Triple300 => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<300>,$crate::codec_reference::storable::Blob<300>)>, "(u64,Blob<300>,Blob<300>)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBytes => ($crate::ReferenceValue<(Vec<u8>,Vec<u8>,Vec<u8>)>, "(Vec<u8>,Vec<u8>,Vec<u8>)", "icss", true, "unbounded", &[0,8], false);
        TripleMixed => ($crate::ReferenceValue<(Vec<u8>,u64,Vec<u8>)>, "(Vec<u8>,u64,Vec<u8>)", "icss", true, "unbounded", &[0,8], false);
        TripleU64BytesBytes => ($crate::ReferenceValue<(u64,Vec<u8>,Vec<u8>)>, "(u64,Vec<u8>,Vec<u8>)", "icss", true, "unbounded", &[0,8], false);
        TripleUUU => ($crate::ReferenceValue<(u64,u64,u64)>, "(u64,u64,u64)", "icss", true, "fixed", &[24], false);
        TripleUUB => ($crate::ReferenceValue<(u64,u64,$crate::codec_reference::storable::Blob<8>)>, "(u64,u64,Blob<8>)", "icss", true, "bounded-variable", &[0,8], false);
        TripleUBU => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<8>,u64)>, "(u64,Blob<8>,u64)", "icss", true, "bounded-variable", &[0,8], false);
        TripleUBB => ($crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>)>, "(u64,Blob<8>,Blob<8>)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBUU => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,u64,u64)>, "(Blob<8>,u64,u64)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBUB => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,u64,$crate::codec_reference::storable::Blob<8>)>, "(Blob<8>,u64,Blob<8>)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBBU => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>,u64)>, "(Blob<8>,Blob<8>,u64)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBBB => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>)>, "(Blob<8>,Blob<8>,Blob<8>)", "icss", true, "bounded-variable", &[0,8], false);
        TripleBBS => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>,String)>, "(Blob<8>,Blob<8>,String)", "icss", true, "unbounded", &[0,8], false);
        TripleBSB => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,String,$crate::codec_reference::storable::Blob<8>)>, "(Blob<8>,String,Blob<8>)", "icss", true, "unbounded", &[0,8], false);
        TripleBSS => ($crate::ReferenceValue<($crate::codec_reference::storable::Blob<8>,String,String)>, "(Blob<8>,String,String)", "icss", true, "unbounded", &[0,8], false);
        TripleSBB => ($crate::ReferenceValue<(String,$crate::codec_reference::storable::Blob<8>,$crate::codec_reference::storable::Blob<8>)>, "(String,Blob<8>,Blob<8>)", "icss", true, "unbounded", &[0,8], false);
        TripleSBS => ($crate::ReferenceValue<(String,$crate::codec_reference::storable::Blob<8>,String)>, "(String,Blob<8>,String)", "icss", true, "unbounded", &[0,8], false);
        TripleSSB => ($crate::ReferenceValue<(String,String,$crate::codec_reference::storable::Blob<8>)>, "(String,String,Blob<8>)", "icss", true, "unbounded", &[0,8], false);
        TripleSSS => ($crate::ReferenceValue<(String,String,String)>, "(String,String,String)", "icss", true, "unbounded", &[0,8], false);
        OptionU64 => (Option<u64>, "Option<u64>", "icss", true, "bounded-variable", &[8], true);
        OptionF64 => (Option<f64>, "Option<f64>", "icss", false, "bounded-variable", &[8], true);
        OptionString => (Option<String>, "Option<String>", "icss", true, "unbounded", &[8], false);
        OptionPairFixed => (Option<$crate::ReferenceValue<(u64,[u8;20])>>, "Option<Shared<(u64,[u8;20])>>", "icss", true, "bounded-variable", &[8], true);
        OptionTripleFixed => (Option<$crate::ReferenceValue<(u64,[u8;20],[u8;20])>>, "Option<Shared<(u64,[u8;20],[u8;20])>>", "icss", true, "bounded-variable", &[8], true);
        OptionTripleBytes => (Option<$crate::ReferenceValue<(Vec<u8>,Vec<u8>,Vec<u8>)>>, "Option<Shared<(Vec<u8>,Vec<u8>,Vec<u8>)>>", "icss", true, "unbounded", &[8], false);
        OptionPair48 => (Option<$crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<48>)>>, "Option<Shared<(u64,Blob<48>)>>", "icss", true, "bounded-variable", &[8], false);
        OptionTriple40_80 => (Option<$crate::ReferenceValue<(u64,$crate::codec_reference::storable::Blob<40>,$crate::codec_reference::storable::Blob<80>)>>, "Option<Shared<(u64,Blob<40>,Blob<80>)>>", "icss", true, "bounded-variable", &[8], false);
        OptionTripleU64BytesBytes => (Option<$crate::ReferenceValue<(u64,Vec<u8>,Vec<u8>)>>, "Option<Shared<(u64,Vec<u8>,Vec<u8>)>>", "icss", true, "unbounded", &[8], false);
        OptionTripleMixed => (Option<$crate::ReferenceValue<(Vec<u8>,u64,Vec<u8>)>>, "Option<Shared<(Vec<u8>,u64,Vec<u8>)>>", "icss", true, "unbounded", &[8], false);
        Unfixed255 => ($crate::ReferenceValue<$crate::catalogue::UnfixedU64<255>>, "ICSS UnfixedU64<255>", "icss", true, "bounded-variable", &[8], false);
        Unfixed65000 => ($crate::ReferenceValue<$crate::catalogue::UnfixedU64<65000>>, "ICSS UnfixedU64<65000>", "icss", true, "bounded-variable", &[8], false);
        Unfixed68000 => ($crate::ReferenceValue<$crate::catalogue::UnfixedU64<68000>>, "ICSS UnfixedU64<68000>", "icss", true, "bounded-variable", &[8], false);
        Principal => ($crate::ReferenceValue<$crate::BenchmarkPrincipal>, "Principal", "icss", true, "bounded-variable", &[1,9,29], false);
        Signed64 => ($crate::Signed64, "Signed64(i64)", "storage", true, "fixed", &[8], false);
        WireRecord => ($crate::WireRecord, "WireRecord", "storage", true, "bounded-variable", &[60], false);
        Session => ($crate::SessionEnvelope, "SessionEnvelope", "storage", true, "bounded-variable", &[256,2048], false);
        Chunk => ($crate::ChunkBytes, "ChunkBytes(max=1MiB)", "storage", true, "bounded-variable", &[0,31,32,33,1024,4096], false);
        AppTuple => ($crate::TupleKey, "Storage TupleKey", "storage", true, "fixed", &[28], false);
        IdentityKey => ($crate::IdentityKey<false>, "Principal key", "storage", true, "bounded-variable", &[31], false);
        CompositeIdentityKey => ($crate::IdentityKey<true>, "Composite Principal key", "storage", true, "bounded-variable", &[62], false);
        AppOptional => ($crate::OptionalU64, "Storage OptionalU64", "storage", true, "bounded-variable", &[9], false);
        Bounded255 => ($crate::BoundedBytes<255>, "Storage BoundedBytes<255>", "storage", true, "bounded-variable", &[0,8,255], false);
        Bounded256 => ($crate::BoundedBytes<256>, "Storage BoundedBytes<256>", "storage", true, "bounded-variable", &[0,8,256], false);
        Bounded65535 => ($crate::BoundedBytes<65535>, "Storage BoundedBytes<65535>", "storage", true, "bounded-variable", &[0,8,256,65535], false);
        Bounded65536 => ($crate::BoundedBytes<65536>, "Storage BoundedBytes<65536>", "storage", true, "bounded-variable", &[0,8,256,65536], false);
    }};
}
macro_rules! define_catalogue {
    ($_context:tt; $($name:ident => ($ty:ty, $label:literal, $source:literal, $ordered:tt, $bound:literal, $payloads:expr, $optional:tt);)*) => {
        /// Concrete data type from the published ICSS tests or the Storage suite.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
        pub enum DataType { $($name,)* }
        pub const DATA_TYPES: &[DataType] = &[$(DataType::$name,)*];
        impl DataType {
            pub const fn name(self)-> &'static str {match self {$(Self::$name=>$label,)*}}
            pub const fn source(self)-> &'static str {match self {$(Self::$name=>$source,)*}}
            pub const fn ordered(self)->bool {match self {$(Self::$name=>$ordered,)*}}
            pub const fn bound(self)-> &'static str {match self {$(Self::$name=>$bound,)*}}
            pub const fn payloads(self)-> &'static [u32] {match self {$(Self::$name=>$payloads,)*}}
            pub const fn fixed_option_refusal(self)->bool {match self {$(Self::$name=>$optional,)*}}
            pub fn expected_value(self,key:u64,payload:u32,replacement:bool)->Vec<u8> {
                match self {$(Self::$name=><$ty as BenchValue>::make(key,payload,replacement).canonical(),)*}
            }
        }
    };
}
type_catalogue!(define_catalogue, ());
impl DataType {
    #[must_use]
    pub const fn payload_scope(self) -> &'static str {
        match self {
            Self::Pair48
            | Self::Pair300
            | Self::TripleU64BytesBytes
            | Self::Triple40_80
            | Self::Triple300
            | Self::TripleBytes
            | Self::TripleMixed
            | Self::TripleUUB
            | Self::TripleUBU
            | Self::TripleUBB
            | Self::TripleBUU
            | Self::TripleBUB
            | Self::TripleBBU
            | Self::TripleBBB
            | Self::TripleBBS
            | Self::TripleBSB
            | Self::TripleBSS
            | Self::TripleSBB
            | Self::TripleSBS
            | Self::TripleSSB
            | Self::TripleSSS => "variable-component-cap",
            Self::OptionU64
            | Self::OptionF64
            | Self::OptionString
            | Self::OptionPairFixed
            | Self::OptionTripleFixed
            | Self::OptionTripleBytes
            | Self::OptionPair48
            | Self::OptionTriple40_80
            | Self::OptionTripleU64BytesBytes
            | Self::OptionTripleMixed => "optional-inner-component-cap",
            _ => "logical-value-bytes",
        }
    }
    #[must_use]
    pub const fn population_limit(self) -> u32 {
        match self {
            Self::Unit => 1,
            Self::Bool => 2,
            Self::U8 => 128,
            Self::U16 => 65_280,
            _ => 1_000_000,
        }
    }
    #[must_use]
    pub const fn tiny(self) -> bool {
        matches!(self, Self::Unit | Self::Bool)
    }
}
/// An ICSS-native codec projected to Storage's existing trait. Collection
/// operations remain monomorphized; normalization validates decoded fields.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReferenceValue<T>(pub T);
impl<T: BenchValue> BenchValue for ReferenceValue<T> {
    fn make(key: u64, payload: u32, replacement: bool) -> Self {
        Self(T::make(key, payload, replacement))
    }
    fn canonical(self) -> Vec<u8> {
        self.0.canonical()
    }
}
impl<T: crate::BenchKey> crate::BenchKey for ReferenceValue<T> {
    fn from_id(id: u64) -> Self {
        Self(T::from_id(id))
    }
    fn id(&self) -> u64 {
        self.0.id()
    }
}
#[cfg(feature = "owned")]
impl<T: crate::codec_reference::Storable> ic_storage::storable::Storable for ReferenceValue<T> {
    const BOUND: ic_storage::storable::Bound = match T::BOUND {
        crate::codec_reference::storable::Bound::Unbounded => {
            ic_storage::storable::Bound::Unbounded
        }
        crate::codec_reference::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        } => ic_storage::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        },
    };
    fn to_bytes(&self) -> Result<std::borrow::Cow<'_, [u8]>, ic_storage::storable::CodecError> {
        Ok(self.0.to_bytes())
    }
    fn into_bytes(self) -> Result<Vec<u8>, ic_storage::storable::CodecError> {
        Ok(self.0.into_bytes())
    }
    fn from_bytes(
        bytes: std::borrow::Cow<'_, [u8]>,
    ) -> Result<Self, ic_storage::storable::CodecError> {
        Ok(Self(T::from_bytes(bytes)))
    }
}
#[cfg(any(
    feature = "dfinity",
    all(target_arch = "wasm32", feature = "dfinity-head")
))]
impl<T: crate::codec_reference::Storable> crate::upstream::Storable for ReferenceValue<T> {
    const BOUND: crate::upstream::storable::Bound = match T::BOUND {
        crate::codec_reference::storable::Bound::Unbounded => {
            crate::upstream::storable::Bound::Unbounded
        }
        crate::codec_reference::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        } => crate::upstream::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        },
    };
    fn to_bytes(&self) -> std::borrow::Cow<'_, [u8]> {
        self.0.to_bytes()
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
    fn from_bytes(bytes: std::borrow::Cow<'_, [u8]>) -> Self {
        Self(T::from_bytes(bytes))
    }
}

pub const ICSS_KEYS: [DataType; 4] = [
    DataType::U32,
    DataType::Blob10,
    DataType::Bytes,
    DataType::String,
];
pub const ICSS_VALUES: [DataType; 5] = [
    DataType::Unit,
    DataType::U32,
    DataType::Blob20,
    DataType::Bytes,
    DataType::String,
];
const EXTRA_PAIRS: &[(DataType, DataType)] = &[
    (DataType::Array32, DataType::Array32),
    (DataType::Array64, DataType::U128),
    (DataType::U128, DataType::U128),
    (DataType::Signed64, DataType::Signed64),
    (DataType::Bounded255, DataType::Bounded255),
    (DataType::Bounded255, DataType::Bytes),
    (DataType::Bytes, DataType::Bounded255),
    (DataType::AppTuple, DataType::AppOptional),
    (DataType::Principal, DataType::Bytes),
    (DataType::IdentityKey, DataType::Bytes),
    (DataType::CompositeIdentityKey, DataType::Bytes),
];
const CAPACITY: usize = DATA_TYPES.len() * 7 + 40 + EXTRA_PAIRS.len() * 2;
const fn build_matrix() -> ([Case; CAPACITY], usize) {
    let blank = Case {
        collection: Collection::Map,
        key: DataType::U64,
        value: DataType::U64,
    };
    let mut result = [blank; CAPACITY];
    let mut count = 0;
    let mut t = 0;
    while t < DATA_TYPES.len() {
        let kind = DATA_TYPES[t];
        let mut c = 0;
        while c < 5 {
            let collection = Collection::ALL[c];
            let (key, value) = if matches!(collection, Collection::Set) {
                (kind, DataType::Unit)
            } else {
                (DataType::U64, kind)
            };
            push_case(&mut result, &mut count, collection, key, value);
            if matches!(collection, Collection::Map | Collection::Region) {
                push_case(&mut result, &mut count, collection, kind, DataType::U64);
            }
            c += 1;
        }
        t += 1;
    }
    let mut k = 0;
    while k < 4 {
        let mut v = 0;
        while v < 5 {
            push_case(
                &mut result,
                &mut count,
                Collection::Map,
                ICSS_KEYS[k],
                ICSS_VALUES[v],
            );
            push_case(
                &mut result,
                &mut count,
                Collection::Region,
                ICSS_KEYS[k],
                ICSS_VALUES[v],
            );
            v += 1;
        }
        k += 1;
    }
    let mut p = 0;
    while p < EXTRA_PAIRS.len() {
        let (key, value) = EXTRA_PAIRS[p];
        push_case(&mut result, &mut count, Collection::Map, key, value);
        push_case(&mut result, &mut count, Collection::Region, key, value);
        p += 1;
    }
    (result, count)
}
const fn push_case(
    out: &mut [Case; CAPACITY],
    n: &mut usize,
    c: Collection,
    k: DataType,
    v: DataType,
) {
    let mut i = 0;
    while i < *n {
        if out[i].collection as u8 == c as u8
            && out[i].key as u8 == k as u8
            && out[i].value as u8 == v as u8
        {
            return;
        }
        i += 1;
    }
    out[*n] = Case {
        collection: c,
        key: k,
        value: v,
    };
    *n += 1;
}
static MATRIX_STORAGE: ([Case; CAPACITY], usize) = build_matrix();
pub const MATRIX_CASES: &[Case] = MATRIX_STORAGE.0.split_at(MATRIX_STORAGE.1).0;

#[must_use]
pub fn operations(case: Case) -> Vec<Operation> {
    use Operation::*;
    let Case {
        collection,
        key,
        value,
    } = case;
    let base = match collection {
        Collection::Map => {
            let mut operations = crate::OPERATIONS.to_vec();
            operations.push(WarmGet);
            operations.push(SecondGet);
            operations
        }
        Collection::Region => vec![
            Control,
            Get,
            MissAbove,
            Replace,
            KeysScan,
            Insert,
            Grow,
            Shrink,
            Remove,
            RemoveMiss,
            Reinsert,
            RemoveInserted,
        ],
        Collection::Set => vec![
            Control,
            Get,
            MissAbove,
            Scan,
            Nth,
            Insert,
            Remove,
            RemoveMiss,
            Reinsert,
            RemoveInserted,
            IterLast,
        ],
        Collection::Vector => vec![
            Control,
            Get,
            MissAbove,
            Set,
            Scan,
            ReverseScan,
            Nth,
            NthBack,
            Last,
            AlternatingScan,
            ScanSet,
            Push,
            Pop,
        ],
        Collection::Cell => vec![Control, Get, Replace, Grow, Shrink],
    };
    base.into_iter()
        .filter(|op| {
            if matches!(op, Grow | Shrink) && value.bound() != "unbounded" {
                return false;
            }
            if matches!(
                collection,
                Collection::Map | Collection::Set | Collection::Region
            ) {
                if key.tiny()
                    && matches!(
                        op,
                        MissAbove
                            | Insert
                            | RemoveMiss
                            | RemoveInserted
                            | BoundedScan
                            | BoundedKeysScan
                    )
                {
                    return false;
                }
                if key == DataType::U8 && *op == RemoveMiss {
                    return false;
                }
            }
            true
        })
        .collect()
}
/// Expected availability is a separately verified result, never a fake timing.
#[must_use]
pub fn unavailable(case: Case, reference: bool) -> Option<(&'static str, &'static str)> {
    let Case {
        collection,
        key,
        value,
    } = case;
    if matches!(
        collection,
        Collection::Map | Collection::Set | Collection::Region
    ) && !key.ordered()
    {
        return Some(("trait", "key type does not implement Ord"));
    }
    if collection == Collection::Vector && value.bound() == "unbounded" {
        return Some(("init", "Vector requires a finite Storable bound"));
    }
    if reference
        && collection != Collection::Cell
        && (value.fixed_option_refusal()
            || matches!(
                collection,
                Collection::Map | Collection::Set | Collection::Region
            ) && key.fixed_option_refusal())
    {
        return Some((
            "setup",
            "ICSS fixed Option bound rejects the one-byte None encoding",
        ));
    }
    None
}
impl Case {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self {
                collection,
                key,
                value,
            } => format!("{}-{key:?}-{value:?}", collection.name()),
        }
    }
}
#[cfg(test)]
mod tests;
