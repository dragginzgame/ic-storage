//! Fixture-owned logical codec, deliberately independent of Canic's Wire format.
use super::Error;
use ic_storage::ordered::Bytes;

/// Explicit application schema; narrowing is an opt-in conversion, never discovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Schema {
    /// Original u64 revision.
    Wide = 1,
    /// Application conversion to a u32 revision, preserving all other fields.
    Narrow = 2,
}

/// Transient logical projection. This Rust struct is never a retained layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Nonzero application identity.
    pub id: u64,
    /// Nonzero revision, checked against the selected schema on encoding.
    pub revision: u64,
    /// At most 29 bytes; anonymous `[4]` is not an owner.
    pub owner: Bytes<29>,
    /// At most 128 UTF-8 bytes, with a nonblank trimmed form.
    pub name: Bytes<128>,
}

pub(super) fn identity(owner: &[u8]) -> Result<(), Error> {
    if owner == [4] {
        Err(Error::Unauthenticated)
    } else if owner.len() > 29 {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}

impl Profile {
    /// Prepare and validate a logical application value without touching a map.
    pub fn new(id: u64, revision: u64, owner: &[u8], name: &str) -> Result<Self, Error> {
        identity(owner)?;
        if id == 0 || revision == 0 || name.len() > 128 || name.trim().is_empty() {
            return Err(Error::Invalid);
        }
        Ok(Self {
            id,
            revision,
            owner: Bytes::new(owner).map_err(|_| Error::Invalid)?,
            name: Bytes::new(name.as_bytes()).map_err(|_| Error::Invalid)?,
        })
    }

    pub(super) fn encode(self, schema: Schema) -> Result<Bytes<512>, Error> {
        let name = core::str::from_utf8(self.name.as_slice()).map_err(|_| Error::Invalid)?;
        Self::new(self.id, self.revision, self.owner.as_slice(), name)?;
        let mut bytes = [0; 185];
        bytes[..8].copy_from_slice(b"PROFILE1");
        bytes[8] = schema as u8;
        bytes[9..17].copy_from_slice(&self.id.to_le_bytes());
        let at = match schema {
            Schema::Wide => {
                bytes[17..25].copy_from_slice(&self.revision.to_le_bytes());
                25
            }
            Schema::Narrow => {
                let revision = u32::try_from(self.revision).map_err(|_| Error::Conversion)?;
                bytes[17..21].copy_from_slice(&revision.to_le_bytes());
                21
            }
        };
        bytes[at] = self.owner.as_slice().len() as u8;
        bytes[at + 1..at + 3].copy_from_slice(&(name.len() as u16).to_le_bytes());
        let end_owner = at + 3 + self.owner.as_slice().len();
        bytes[at + 3..end_owner].copy_from_slice(self.owner.as_slice());
        bytes[end_owner..end_owner + name.len()].copy_from_slice(name.as_bytes());
        Bytes::new(&bytes[..end_owner + name.len()]).map_err(Error::Storage)
    }

    pub(super) fn decode(bytes: &[u8], schema: Schema) -> Result<Self, Error> {
        let at = match schema {
            Schema::Wide => 25,
            Schema::Narrow => 21,
        };
        if bytes.len() < at + 3 || &bytes[..8] != b"PROFILE1" || bytes[8] != schema as u8 {
            return Err(Error::Invalid);
        }
        let id = u64::from_le_bytes(bytes[9..17].try_into().map_err(|_| Error::Invalid)?);
        let revision = match schema {
            Schema::Wide => {
                u64::from_le_bytes(bytes[17..25].try_into().map_err(|_| Error::Invalid)?)
            }
            Schema::Narrow => u64::from(u32::from_le_bytes(
                bytes[17..21].try_into().map_err(|_| Error::Invalid)?,
            )),
        };
        let owner_len = usize::from(bytes[at]);
        let name_len = usize::from(u16::from_le_bytes([bytes[at + 1], bytes[at + 2]]));
        if owner_len > 29 || name_len > 128 || bytes.len() != at + 3 + owner_len + name_len {
            return Err(Error::Invalid);
        }
        let name =
            core::str::from_utf8(&bytes[at + 3 + owner_len..]).map_err(|_| Error::Invalid)?;
        Self::new(id, revision, &bytes[at + 3..at + 3 + owner_len], name)
    }
}
