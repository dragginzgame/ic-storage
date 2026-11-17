//! Closed fixture binding of four independently granted public native roots.
use super::{Element, ordered_family, ordered_runtime, region::pool::Pool};
use alloc::vec::Vec;
use ic_storage::ordered::{
    AllocationUsage, Map,
    session::{Error, Participant, Read, Session, Write},
};

type Nodes = &'static Pool<'static, Element>;
type Payload = &'static Pool<'static, u8>;
pub(super) type Native = Map<512, 16384, Nodes, Payload>;
type Owned = (
    Session<512, 16384, Nodes, Payload>,
    [Participant<512, 16384, Nodes, Payload>; 4],
);
pub(super) use ic_storage_native_feasibility::migration_job::ordered_family::ROOTS;

pub(super) struct Owner {
    owned: Option<Owned>,
}
impl Owner {
    pub(super) fn new(maps: [Native; 4]) -> Self {
        ordered_runtime::metadata(|| {
            let maps = ROOTS.into_iter().zip(maps).collect();
            let mut session = Session::from_maps(maps, &ROOTS).unwrap();
            let grants = core::array::from_fn(|i| session.take(ROOTS[i].participant).unwrap());
            Self {
                owned: Some((session, grants)),
            }
        })
    }
    pub(super) fn read<R>(
        &self,
        operation: impl for<'a> FnOnce(Read<'a, 512, 16384, Nodes, Payload>) -> R,
    ) -> Result<R, Error> {
        self.owned.as_ref().expect("live owner").1[0].read(operation)
    }
    pub(super) fn update<R>(
        &mut self,
        operation: impl for<'a> FnOnce(Write<'a, 512, 16384, Nodes, Payload>) -> R,
    ) -> Result<R, Error> {
        self.owned.as_mut().expect("live owner").1[0].update(operation)
    }
    pub(super) fn read_all<R>(
        &self,
        operation: impl for<'a> FnOnce(Read<'a, 512, 16384, Nodes, Payload>) -> R,
    ) -> Result<R, Error> {
        let (session, grants) = self.owned.as_ref().expect("live owner");
        session.read(&grants.each_ref(), operation)
    }
    pub(super) fn compose<R>(
        &mut self,
        operation: impl for<'a> FnOnce(Write<'a, 512, 16384, Nodes, Payload>) -> R,
    ) -> Result<R, Error> {
        let (session, grants) = self.owned.as_mut().expect("live owner");
        session.update(&grants.each_ref(), operation)
    }
    pub(super) fn checkpoint(&self, epoch: u64) -> Result<Vec<u8>, Error> {
        self.read(|view| view.checkpoint(1, epoch))?
    }
    pub(super) fn export(&self, epoch: u64) -> Vec<u8> {
        let parts = self
            .read_all(|view| ROOTS.map(|root| view.checkpoint(root.id, epoch).unwrap()))
            .unwrap();
        ordered_family::encode(epoch, &parts)
    }
    pub(super) fn allocation_usage(&self) -> Result<AllocationUsage, Error> {
        self.read_all(|view| {
            let mut total = AllocationUsage {
                initialized_bytes: 0,
                allocated_bytes: 0,
            };
            for root in ROOTS {
                let usage = view.usage(root.id)?;
                total.initialized_bytes += usage.initialized_bytes;
                total.allocated_bytes += usage.allocated_bytes;
            }
            Ok(total)
        })?
    }
    fn end(&mut self) -> [Native; 4] {
        let (session, grants) = self.owned.take().expect("live owner");
        drop(grants);
        let maps = {
            let maps = session
                .into_maps()
                .unwrap_or_else(|_| panic!("outstanding or poisoned grants"));
            assert_eq!(maps.len(), 4);
            let mut maps = maps.into_iter();
            core::array::from_fn(|_| maps.next().unwrap().1)
        };
        ordered_runtime::release_metadata();
        maps
    }
    pub(super) fn release(mut self) -> [Native; 4] {
        self.end()
    }
    pub(super) fn restore(&mut self, checkpoint: &[u8], epoch: u64) {
        let parts = ordered_family::decode(checkpoint, epoch);
        let mut maps = self.end();
        for (map, bytes) in maps.iter_mut().zip(parts) {
            map.restore(bytes, epoch).unwrap();
        }
        *self = Self::new(maps);
    }
}
