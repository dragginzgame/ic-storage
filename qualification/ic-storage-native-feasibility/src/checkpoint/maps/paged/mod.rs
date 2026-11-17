//! Bind two product page chains to one epoch and complete checkpoint identity.
//! Durable job progress and native publication remain outside this byte codec.
use ic_storage::map::{
    Error, ExportError, Map,
    paged::{MANIFEST_BYTES as ROOT_BYTES, Manifest as RootManifest, PAGE_BYTES},
};
use sha2::{Digest, Sha256};

/// Complete two-root manifest: header, two product manifests, seal and digest.
pub const MANIFEST_BYTES: usize = 72 + 2 * ROOT_BYTES;
const MAGIC: &[u8; 8] = b"NKPPAIR1";
const SEAL: &[u8; 8] = b"PAIRDONE";
const END: usize = 32 + 2 * ROOT_BYTES;

/// Admitted pair with root identities 1 and 2 in that order, at one epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    epoch: u64,
    roots: [RootManifest; 2],
}

impl Manifest {
    /// Admit complete framing, both root identities, epoch and nested integrity.
    /// `limit` applies independently to each root. Page data is validated later
    /// by each product reader; admitting this manifest does not publish a map.
    pub fn decode(bytes: &[u8], epoch: u64, limit: usize) -> Result<Self, Error> {
        if bytes.len() != MANIFEST_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..16] != 1u64.to_le_bytes()
            || bytes[24..32] != 2u64.to_le_bytes()
            || &bytes[END..END + 8] != SEAL
        {
            return Err(Error::Checkpoint);
        }
        if bytes[16..24] != epoch.to_le_bytes() {
            return Err(Error::Epoch);
        }
        if Sha256::digest(&bytes[..END + 8]).as_slice() != &bytes[END + 8..] {
            return Err(Error::Integrity);
        }
        let roots = [
            RootManifest::decode(&bytes[32..32 + ROOT_BYTES], epoch, 1, limit)?,
            RootManifest::decode(&bytes[32 + ROOT_BYTES..END], epoch, 2, limit)?,
        ];
        roots[0]
            .len()
            .checked_add(roots[1].len())
            .ok_or(Error::Limit)?;
        Ok(Self { epoch, roots })
    }

    /// Canonical pair bytes. Publish only after both referenced chains are stored.
    #[must_use]
    pub fn encode(&self) -> [u8; MANIFEST_BYTES] {
        let mut bytes = [0; MANIFEST_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.epoch.to_le_bytes());
        bytes[24..32].copy_from_slice(&2u64.to_le_bytes());
        for (index, root) in self.roots.iter().enumerate() {
            let start = 32 + index * ROOT_BYTES;
            bytes[start..start + ROOT_BYTES].copy_from_slice(&root.encode());
        }
        bytes[END..END + 8].copy_from_slice(SEAL);
        let digest = Sha256::digest(&bytes[..END + 8]);
        bytes[END + 8..].copy_from_slice(&digest);
        bytes
    }

    /// Independent verified root manifests, always in root-1/root-2 order.
    #[must_use]
    pub const fn roots(&self) -> &[RootManifest; 2] {
        &self.roots
    }

    /// Combined page count; admission proves the sum is representable.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.roots[0].page_count() + self.roots[1].page_count()
    }
}

/// Export all root-1 pages, then root-2 pages, descending by index within each.
/// Returns a pair manifest only after both exports succeed. Partial sink writes
/// are unsealed and must not replace a previously published checkpoint.
pub fn export<E>(
    maps: &[Map; 2],
    epoch: u64,
    mut sink: impl FnMut(u64, usize, &[u8; PAGE_BYTES]) -> Result<(), E>,
) -> Result<Manifest, ExportError<E>> {
    let lengths = [
        maps[0].len().map_err(ExportError::Map)?,
        maps[1].len().map_err(ExportError::Map)?,
    ];
    lengths[0]
        .checked_add(lengths[1])
        .ok_or(ExportError::Map(Error::Limit))?;
    let roots = [
        maps[0].export_pages(epoch, 1, |index, page| sink(1, index, page))?,
        maps[1].export_pages(epoch, 2, |index, page| sink(2, index, page))?,
    ];
    Ok(Manifest { epoch, roots })
}

#[cfg(test)]
mod tests;
