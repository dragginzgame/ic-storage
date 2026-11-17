//! Byte-defined root custody directory. No private Rust header is retained.
use super::{DIRECTORY_BYTES, Error, Geometry, MAX_ROOTS, Root, pool::Span};
use sha2::{Digest, Sha256};
pub(super) const MAGIC: &[u8; 8] = b"ICRET001";
#[derive(Clone, Copy, Default)]
pub(super) struct Image {
    pub nodes: Span,
    pub bytes: Span,
    pub root: u64,
    pub free: u64,
    pub len: usize,
}
fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
fn declarations(root: Root) -> [u64; 6] {
    [
        root.id,
        root.participant,
        root.schema,
        root.entries as u64,
        root.key_bytes as u64,
        root.value_bytes as u64,
    ]
}
pub(super) fn encode(
    geometry: Geometry,
    roots: &[Root],
    images: &[Image],
    closed: bool,
) -> [u8; DIRECTORY_BYTES] {
    let mut bytes = [0; DIRECTORY_BYTES];
    bytes[..8].copy_from_slice(MAGIC);
    for (index, value) in [
        1,
        geometry.bank.base() as u64,
        geometry.nodes as u64,
        geometry.bytes as u64,
        roots.len() as u64,
        u64::from(closed),
    ]
    .into_iter()
    .enumerate()
    {
        bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    for (i, (root, image)) in roots.iter().zip(images).enumerate() {
        let fields = [
            image.nodes.start as u64,
            image.nodes.len as u64,
            image.nodes.cap as u64,
            image.bytes.start as u64,
            image.bytes.len as u64,
            image.bytes.cap as u64,
            image.root,
            image.free,
            image.len as u64,
        ];
        for (field, value) in declarations(*root).into_iter().chain(fields).enumerate() {
            let at = 64 + i * 120 + field * 8;
            bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    let end = DIRECTORY_BYTES - 32;
    let seal = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&seal);
    bytes
}
pub(super) fn decode(
    bytes: &[u8; DIRECTORY_BYTES],
    geometry: Geometry,
    roots: &[Root],
) -> Result<[Image; MAX_ROOTS], Error> {
    let end = DIRECTORY_BYTES - 32;
    if &bytes[..8] != MAGIC
        || word(bytes, 8) != 1
        || word(bytes, 16) != geometry.bank.base() as u64
        || word(bytes, 24) != geometry.nodes as u64
        || word(bytes, 32) != geometry.bytes as u64
        || word(bytes, 40) != roots.len() as u64
        || word(bytes, 48) != 1
        || Sha256::digest(&bytes[..end]).as_slice() != &bytes[end..]
    {
        return Err(Error::Directory);
    }
    let mut images = [Image::default(); MAX_ROOTS];
    for (i, root) in roots.iter().enumerate() {
        let at = 64 + i * 120;
        if declarations(*root)
            .into_iter()
            .enumerate()
            .any(|(j, v)| word(bytes, at + j * 8) != v)
        {
            return Err(Error::Declaration);
        }
        let span = |at| -> Result<Span, Error> {
            let s = Span {
                start: word(bytes, at).try_into().map_err(|_| Error::Geometry)?,
                len: word(bytes, at + 8)
                    .try_into()
                    .map_err(|_| Error::Geometry)?,
                cap: word(bytes, at + 16)
                    .try_into()
                    .map_err(|_| Error::Geometry)?,
            };
            if s.cap == 0 && (s.start != 0 || s.len != 0) {
                return Err(Error::Geometry);
            }
            Ok(s)
        };
        let image = Image {
            nodes: span(at + 48)?,
            bytes: span(at + 72)?,
            root: word(bytes, at + 96),
            free: word(bytes, at + 104),
            len: word(bytes, at + 112)
                .try_into()
                .map_err(|_| Error::Geometry)?,
        };
        if image.nodes.cap == 0
            && (image.bytes.cap != 0 || image.root != 0 || image.free != 0 || image.len != 0)
        {
            return Err(Error::Directory);
        }
        images[i] = image;
    }
    if encode(geometry, roots, &images[..roots.len()], true) != *bytes {
        return Err(Error::Directory);
    }
    Ok(images)
}
