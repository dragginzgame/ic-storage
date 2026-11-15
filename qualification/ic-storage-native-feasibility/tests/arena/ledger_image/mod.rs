//! Controlled metadata bytes, never an allocation or pointer admission token.

use sha2::{Digest, Sha256};

use super::{Block, Ledger, WORDS};

const HEADER: usize = 32;
const SLOT: usize = 40;
const DIGEST: usize = HEADER + WORDS * SLOT;
pub(super) const IMAGE_BYTES: usize = DIGEST + 32;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Length,
    Format,
    Integrity,
    Encoding,
    Ledger,
    ActiveOwner,
}

// This codec is intentionally private to the experiment. In particular, there
// is no operation accepting its output as an Arena's trusted live bookkeeping.
pub(super) fn encode(ledger: &Ledger) -> Result<[u8; IMAGE_BYTES], Error> {
    ledger.validate().map_err(|_| Error::Ledger)?;
    let mut bytes = [0; IMAGE_BYTES];
    bytes[..8].copy_from_slice(b"ICNLED01");
    put(&mut bytes, 8, 1);
    put(&mut bytes, 16, WORDS as u64);
    put(&mut bytes, 24, ledger.next_generation);
    for (index, entry) in ledger.blocks.iter().enumerate() {
        let Some(block) = entry else { continue };
        let length = block.parked_len.ok_or(Error::ActiveOwner)?;
        let offset = HEADER + index * SLOT;
        for (field, value) in [
            1,
            block.start as u64,
            block.words as u64,
            block.generation,
            length as u64,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut bytes, offset + field * 8, value);
        }
    }
    let digest = Sha256::digest(&bytes[..DIGEST]);
    bytes[DIGEST..].copy_from_slice(&digest);
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Ledger, Error> {
    if bytes.len() != IMAGE_BYTES {
        return Err(Error::Length);
    }
    if &bytes[..8] != b"ICNLED01" || word(bytes, 8) != 1 || word(bytes, 16) != WORDS as u64 {
        return Err(Error::Format);
    }
    if Sha256::digest(&bytes[..DIGEST]).as_slice() != &bytes[DIGEST..] {
        return Err(Error::Integrity);
    }
    let mut ledger = Ledger {
        occupied: [false; WORDS],
        blocks: [None; WORDS],
        next_generation: word(bytes, 24),
    };
    for index in 0..WORDS {
        let offset = HEADER + index * SLOT;
        match word(bytes, offset) {
            0 => {
                if bytes[offset..offset + SLOT].iter().any(|byte| *byte != 0) {
                    return Err(Error::Encoding);
                }
            }
            1 => {
                let start = usize::try_from(word(bytes, offset + 8)).map_err(|_| Error::Ledger)?;
                let words = usize::try_from(word(bytes, offset + 16)).map_err(|_| Error::Ledger)?;
                let length =
                    usize::try_from(word(bytes, offset + 32)).map_err(|_| Error::Ledger)?;
                let end = start
                    .checked_add(words)
                    .filter(|end| *end <= WORDS)
                    .ok_or(Error::Ledger)?;
                ledger.blocks[index] = Some(Block {
                    start,
                    words,
                    generation: word(bytes, offset + 24),
                    parked_len: Some(length),
                });
                ledger.occupied[start..end].fill(true);
            }
            _ => return Err(Error::Encoding),
        }
    }
    ledger.validate().map_err(|_| Error::Ledger)?;
    Ok(ledger)
}

fn word(bytes: &[u8], offset: usize) -> u64 {
    // Only fixed offsets within the admitted exact-size image reach this helper.
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests;
