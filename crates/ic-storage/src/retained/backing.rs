//! The only source of allocation provenance: owned host allocations or the
//! supplier-reserved Wasm windows admitted by retained_build before installation.
use super::{DIRECTORY_BYTES, Error, Geometry, Node};
#[cfg(not(target_arch = "wasm32"))]
use alloc::alloc::alloc_zeroed;
use alloc::{
    alloc::{Layout, dealloc},
    rc::Rc,
};
use core::{cell::Cell, ptr::NonNull};

pub(super) const HEADER: usize = 1_572_864;
pub(super) const A: usize = 2 * 1024 * 1024;
pub(super) const B: usize = 18 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
pub(super) const PAYLOAD: usize = 1024 * 1024;
pub(super) struct Backing {
    pub geometry: Geometry,
    pub nodes: NonNull<u8>,
    pub bytes: NonNull<u8>,
    header: Cell<[u8; DIRECTORY_BYTES]>,
    owned: bool,
}
impl Backing {
    #[expect(
        unsafe_code,
        reason = "fresh creation inspects initialized reserved backing before any native loan"
    )]
    pub fn fresh(&self) -> bool {
        if self.header() != [0; DIRECTORY_BYTES] {
            return false;
        }
        // SAFETY: construction has not issued any map/pool yet. Host allocations
        // are zero initialized; the admitted Wasm windows are initialized memory.
        // Check both reserved banks on Wasm, not just the requested capacity.
        unsafe {
            if self.owned {
                core::slice::from_raw_parts(
                    self.nodes.as_ptr(),
                    self.geometry.nodes * size_of::<Node>(),
                )
                .iter()
                .all(|b| *b == 0)
                    && core::slice::from_raw_parts(self.bytes.as_ptr(), self.geometry.bytes)
                        .iter()
                        .all(|b| *b == 0)
            } else {
                core::slice::from_raw_parts(
                    core::ptr::with_exposed_provenance::<u64>(A),
                    32 * 1024 * 1024 / 8,
                )
                .iter()
                .all(|word| *word == 0)
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[expect(
        unsafe_code,
        reason = "owned zero-initialized aligned host allocation provenance"
    )]
    pub fn host(geometry: Geometry) -> Result<Rc<Self>, Error> {
        geometry.check()?;
        let node_layout = Layout::array::<Node>(geometry.nodes).map_err(|_| Error::Geometry)?;
        let byte_layout = Layout::array::<u8>(geometry.bytes).map_err(|_| Error::Geometry)?;
        // SAFETY: checked nonzero layouts; all-zero Node consists only of u64s.
        // These allocations remain owned by this Rc until every pool/map ends.
        unsafe {
            let nodes = NonNull::new(alloc_zeroed(node_layout)).ok_or(Error::Allocation)?;
            let Some(bytes) = NonNull::new(alloc_zeroed(byte_layout)) else {
                dealloc(nodes.as_ptr(), node_layout);
                return Err(Error::Allocation);
            };
            Ok(Rc::new(Self {
                geometry,
                nodes,
                bytes,
                header: Cell::new([0; DIRECTORY_BYTES]),
                owned: true,
            }))
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn wasm(geometry: Geometry) -> Result<Rc<Self>, Error> {
        geometry.check()?;
        let base = geometry.bank.base();
        // No caller pointer or competing allocation authority enters this API.
        // The one canister singleton and admitted memory layout own these windows.
        Ok(Rc::new(Self {
            geometry,
            nodes: NonNull::new(core::ptr::with_exposed_provenance_mut(base))
                .ok_or(Error::Geometry)?,
            bytes: NonNull::new(core::ptr::with_exposed_provenance_mut(base + PAYLOAD))
                .ok_or(Error::Geometry)?,
            header: Cell::new([0; DIRECTORY_BYTES]),
            owned: false,
        }))
    }
    #[expect(
        unsafe_code,
        reason = "copy the supplier-owned initialized scalar directory"
    )]
    pub fn header(&self) -> [u8; DIRECTORY_BYTES] {
        if self.owned {
            self.header.get()
        } else {
            // SAFETY: Wasm contract reserves this initialized disjoint interval;
            // no reference into it is ever handed out and runtime access is scoped.
            unsafe { core::ptr::read(core::ptr::with_exposed_provenance(HEADER)) }
        }
    }
    #[expect(
        unsafe_code,
        reason = "publish the supplier directory after ownership transitions"
    )]
    pub fn publish(&self, header: [u8; DIRECTORY_BYTES]) {
        if self.owned {
            self.header.set(header);
        } else {
            // SAFETY: same exclusive directory contract as header().
            unsafe {
                core::ptr::write(core::ptr::with_exposed_provenance_mut(HEADER), header);
            }
        }
    }
}
impl Drop for Backing {
    #[expect(
        unsafe_code,
        reason = "release owned host allocations only after the last allocator reference"
    )]
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: Rc keeps the backing alive through every Vec/Pool; these
            // exact layouts created the original allocations, never relocated.
            unsafe {
                dealloc(
                    self.nodes.as_ptr(),
                    Layout::array::<Node>(self.geometry.nodes).unwrap(),
                );
                dealloc(
                    self.bytes.as_ptr(),
                    Layout::array::<u8>(self.geometry.bytes).unwrap(),
                );
            }
        }
    }
}
