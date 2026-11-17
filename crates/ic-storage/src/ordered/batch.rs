use super::{Error, View, arena};
use core::alloc::Allocator;

/// One encoded mutation in an ordered, bounded batch.
#[derive(Clone, Copy)]
pub enum Mutation<'a> {
    /// Insert or replace exact bytes.
    Put(&'a [u8], &'a [u8]),
    /// Delete if present; absence is a successful no-op.
    Remove(&'a [u8]),
}

impl Mutation<'_> {
    fn key(&self) -> &[u8] {
        match self {
            Self::Put(key, _) | Self::Remove(key) => key,
        }
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    View<'_, K, V, A, P>
{
    /// Apply at most 2,048 physical mutations after complete bounds/capacity preflight.
    /// A DB entity mutation may expand into many row/index mutations; this is not
    /// the consumer's entity-batch limit. Preflight is quadratic in batch length.
    /// This supports rows and secondary entries in one collection. It does not
    /// infer uniqueness/reference policy or span independently owned roots.
    /// A host unwind still poisons the owner; installed trap rollback is runtime-owned.
    ///
    /// # Errors
    /// Excessive batch size, byte bounds, intermediate live-entry limit or allocator
    /// refusal preserve every record. Duplicate keys follow sequential semantics.
    pub fn apply(&mut self, mutations: &[Mutation<'_>]) -> Result<(), Error> {
        if mutations.len() > 2048 {
            return Err(Error::Limit);
        }
        let mut live = self.len();
        let mut peak = live;
        let mut payload = arena::Plan::new(&self.map.image.payload);
        for (position, mutation) in mutations.iter().enumerate() {
            if mutation.key().len() > K
                || matches!(mutation, Mutation::Put(_, value) if value.len() > V)
            {
                return Err(Error::Limit);
            }
            let previous = mutations[..position]
                .iter()
                .rev()
                .find(|prior| prior.key() == mutation.key())
                .map_or_else(
                    || self.get(mutation.key()).map(|value| value.as_slice().len()),
                    |prior| match prior {
                        Mutation::Put(_, value) => Some(value.len()),
                        Mutation::Remove(_) => None,
                    },
                );
            let present = previous.is_some();
            match (mutation, previous) {
                (Mutation::Put(key, value), None) => {
                    payload.allocate(key.len())?;
                    payload.allocate(value.len())?;
                }
                (Mutation::Put(_, value), Some(old)) => payload.replace(old, value.len())?,
                (Mutation::Remove(key), Some(old)) => {
                    payload.retire(key.len())?;
                    payload.retire(old)?;
                }
                _ => {}
            }
            match (mutation, present) {
                (Mutation::Put(..), false) => live = live.checked_add(1).ok_or(Error::Limit)?,
                (Mutation::Remove(..), true) => live -= 1,
                _ => {}
            }
            if live > self.map.limit {
                return Err(Error::Limit);
            }
            peak = peak.max(live);
        }
        let additional = peak.saturating_sub(self.map.image.nodes.len());
        self.map
            .image
            .nodes
            .try_reserve(additional)
            .map_err(|_| Error::Allocation)?;
        payload.reserve(&mut self.map.image.payload)?;
        for mutation in mutations {
            match mutation {
                Mutation::Put(key, value) => {
                    self.insert(key, value)?;
                }
                Mutation::Remove(key) => {
                    self.remove(key);
                }
            }
        }
        Ok(())
    }
}
