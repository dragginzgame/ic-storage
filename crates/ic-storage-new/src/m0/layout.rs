//! Fixed M0 v1 constants. Retained bytes never select these bounds or schemas.

pub(super) const PAGE: usize = 65536;
pub(super) const META_BASE: usize = 8388608;
pub(super) const ARENA_BASE: usize = META_BASE + PAGE;
pub(super) const MEMORY_LIMIT: usize = 16777216;
pub(super) const INDEX_BASE: usize = 1024;
pub(super) const INDEX_STRIDE: usize = 32;
pub(super) const CAPACITY: usize = 1024;
pub(super) const INDEX_END: usize = INDEX_BASE + CAPACITY * INDEX_STRIDE;
pub(super) const COUNTER: &[u8; 16] = b"op.m0.counter.v1";
pub(super) const NODE: &[u8; 16] = b"op.m0.node.v1\0\0\0";
pub(super) const DOMAIN: &[u8] = b"ic-storage-new/m0/metadata/v1\0";

const fn copy<const N: usize>(dst: &mut [u8; 256], offset: usize, src: &[u8; N]) {
    let mut i = 0;
    while i < N {
        dst[offset + i] = src[i];
        i += 1;
    }
}

const fn descriptors() -> [u8; 256] {
    let mut bytes = [0; 256];
    let mut schema = 0;
    while schema < 2 {
        let base = schema * 128;
        let (id, size, fields) = if schema == 0 {
            (COUNTER, 40u32, 3u32)
        } else {
            (NODE, 24, 2)
        };
        copy(&mut bytes, base, id);
        copy(&mut bytes, base + 16, &size.to_le_bytes());
        copy(&mut bytes, base + 20, &8u32.to_le_bytes());
        copy(&mut bytes, base + 24, &fields.to_le_bytes());
        let mut field = 0;
        while field < fields {
            let at = base + 32 + field as usize * 32;
            let (offset, kind, width) = if field == 0 {
                (0u32, 1u32, 8u32)
            } else {
                (8 + (field - 1) * 16, 2, 16)
            };
            copy(&mut bytes, at, &(field + 1).to_le_bytes());
            copy(&mut bytes, at + 4, &offset.to_le_bytes());
            copy(&mut bytes, at + 8, &kind.to_le_bytes());
            copy(&mut bytes, at + 12, &width.to_le_bytes());
            if field != 0 {
                copy(&mut bytes, at + 16, NODE);
            }
            field += 1;
        }
        schema += 1;
    }
    bytes
}

pub(super) const DESCRIPTORS: [u8; 256] = descriptors();
