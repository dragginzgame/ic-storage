//! Shared node search; the caller supplies committed or staged page reads.
use super::{
    Comparator, Error,
    page::{self, Node, Page, Record},
};
use std::cmp::Ordering;

pub(super) fn locate(
    node: &Node,
    key: &[u8],
    order: &impl Comparator,
    mut read: impl FnMut(u64, &mut Page) -> Result<(), Error>,
    mut load_key: impl FnMut(Record) -> Result<Vec<u8>, Error>,
) -> Result<(usize, bool), Error> {
    let (mut lo, mut hi) = (0, node.entries.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let record = node.entries[mid];
        let order = if record.key != 0 && record.key_len <= page::PAYLOAD as u64 {
            let mut buffer = [0; page::SIZE];
            read(record.key, &mut buffer)?;
            order.compare(page::single_blob(&buffer, record.key_len)?, key)?
        } else {
            order.compare(&load_key(record)?, key)?
        };
        match order {
            Ordering::Less => lo = mid + 1,
            Ordering::Equal => return Ok((mid, true)),
            Ordering::Greater => hi = mid,
        }
    }
    Ok((lo, false))
}
