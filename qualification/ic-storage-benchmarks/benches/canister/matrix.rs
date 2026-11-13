//! Concrete dispatch for the catalogue. No type erasure enters collection IO.
#[expect(
    clippy::wildcard_imports,
    reason = "Private canister modules share parent bindings selected by the benchmark backend features."
)]
use super::*;

fn pair<K: BenchKey + Storable + 'static, V: BenchValue + Storable + 'static>(
    case: Case,
    collection: Collection,
    settings: Initialization,
) -> Box<dyn Benchmark> {
    match collection {
        Collection::Map => Box::new(TypedMap::<K, V> {
            case,
            map: configured_map(settings),
            settings,
        }),
        Collection::Region => Box::new(Region::<K, V>::new(case, settings.upstream_bucket_pages)),
        _ => unreachable!(),
    }
}
fn by_value<V: BenchValue + Storable + 'static>(
    case: Case,
    collection: Collection,
    settings: Initialization,
) -> Box<dyn Benchmark> {
    match collection {
        Collection::Map | Collection::Region => pair::<u64, V>(case, collection, settings),
        Collection::Cell => Box::new(TypedCell::<V>::new(case, settings.upstream_bucket_pages)),
        Collection::Vector => Box::new(TypedVector::<V>::new(case, settings.upstream_bucket_pages)),
        Collection::Set => unreachable!(),
    }
}
fn by_key<K: BenchKey + Storable + 'static>(
    case: Case,
    collection: Collection,
    settings: Initialization,
) -> Box<dyn Benchmark> {
    match collection {
        Collection::Map | Collection::Region => pair::<K, u64>(case, collection, settings),
        Collection::Set => Box::new(TypedSet::<K>::new(case, settings.upstream_bucket_pages)),
        _ => unreachable!(),
    }
}
macro_rules! values {
    (($kind:expr,$case:expr,$collection:expr,$settings:expr); $($name:ident => ($ty:ty,$label:literal,$source:literal,$ord:tt,$bound:literal,$payloads:expr,$opt:tt);)*) => {
        match $kind {$(DataType::$name => by_value::<$ty>($case,$collection,$settings),)*}
    };
}
macro_rules! key_type {
    (true,$ty:ty,$case:expr,$collection:expr,$settings:expr) => {
        by_key::<$ty>($case, $collection, $settings)
    };
    (false,$ty:ty,$case:expr,$collection:expr,$settings:expr) => {
        panic!("key type does not implement Ord")
    };
}
macro_rules! keys {
    (($kind:expr,$case:expr,$collection:expr,$settings:expr); $($name:ident => ($ty:ty,$label:literal,$source:literal,$ord:tt,$bound:literal,$payloads:expr,$opt:tt);)*) => {
        match $kind {$(DataType::$name => key_type!($ord,$ty,$case,$collection,$settings),)*}
    };
}
fn grid<K: BenchKey + Storable + 'static>(
    case: Case,
    collection: Collection,
    value: DataType,
    settings: Initialization,
) -> Box<dyn Benchmark> {
    match value {
        DataType::Unit => pair::<K, ()>(case, collection, settings),
        DataType::U32 => pair::<K, u32>(case, collection, settings),
        DataType::Blob20 => pair::<K, ReferenceValue<codec_reference::storable::Blob<20>>>(
            case, collection, settings,
        ),
        DataType::Bytes => pair::<K, Vec<u8>>(case, collection, settings),
        DataType::String => pair::<K, String>(case, collection, settings),
        _ => unreachable!(),
    }
}
pub(super) fn create(
    case: Case,
    collection: Collection,
    key: DataType,
    value: DataType,
    settings: Initialization,
) -> Box<dyn Benchmark> {
    if collection == Collection::Set
        || value == DataType::U64 && matches!(collection, Collection::Map | Collection::Region)
    {
        return ic_storage_benchmarks::type_catalogue!(keys, (key, case, collection, settings));
    }
    if key == DataType::U64 {
        return ic_storage_benchmarks::type_catalogue!(values, (value, case, collection, settings));
    }
    match (key, value) {
        (DataType::U32, _) => grid::<u32>(case, collection, value, settings),
        (DataType::Blob10, _) => grid::<ReferenceValue<codec_reference::storable::Blob<10>>>(
            case, collection, value, settings,
        ),
        (DataType::Bytes, DataType::Bounded255) => {
            pair::<Vec<u8>, BoundedBytes<255>>(case, collection, settings)
        }
        (DataType::Bytes, _) => grid::<Vec<u8>>(case, collection, value, settings),
        (DataType::String, _) => grid::<String>(case, collection, value, settings),
        (DataType::Array32, DataType::Array32) => {
            pair::<[u8; 32], [u8; 32]>(case, collection, settings)
        }
        (DataType::Array64, DataType::U128) => pair::<[u8; 64], u128>(case, collection, settings),
        (DataType::U128, DataType::U128) => pair::<u128, u128>(case, collection, settings),
        (DataType::Signed64, DataType::Signed64) => {
            pair::<Signed64, Signed64>(case, collection, settings)
        }
        (DataType::Bounded255, DataType::Bounded255) => {
            pair::<BoundedBytes<255>, BoundedBytes<255>>(case, collection, settings)
        }
        (DataType::Bounded255, DataType::Bytes) => {
            pair::<BoundedBytes<255>, Vec<u8>>(case, collection, settings)
        }
        (DataType::AppTuple, DataType::AppOptional) => {
            pair::<TupleKey, OptionalU64>(case, collection, settings)
        }
        (DataType::Principal, DataType::Bytes) => {
            pair::<ReferenceValue<BenchmarkPrincipal>, Vec<u8>>(case, collection, settings)
        }
        (DataType::IdentityKey, DataType::Bytes) => {
            pair::<IdentityKey<false>, Vec<u8>>(case, collection, settings)
        }
        (DataType::CompositeIdentityKey, DataType::Bytes) => {
            pair::<IdentityKey<true>, Vec<u8>>(case, collection, settings)
        }
        _ => unreachable!("unregistered key/value pair"),
    }
}
