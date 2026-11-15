//! Host-only experiment for scoped ownership of one integer record and vector.
//!
//! This uses live, ordinary Global allocations. It neither retains a Vec header
//! nor reopens pointers. The checkpoint module exercises bounded logical export
//! and import, separately from unresolved native and installed lifecycle admission.
//! This is fixture code,
//! not the proposed product collection API.
#![forbid(unsafe_code)]

use std::marker::PhantomData;

pub mod checkpoint;

/// Bounded fixture size; resource refusal must precede mutation.
pub const MAX_VALUES: usize = 16;

/// Expected operation refusal; earlier successful writes remain committed.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The vector is currently detached.
    Detached,
    /// The fixture's declared element limit was reached.
    Capacity,
    /// Global allocation failed before the append.
    Allocation,
}

/// A failed frame cannot be reopened; native poisoning is not rollback.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// An earlier panic or unfinished ownership operation quarantined this owner.
    Poisoned,
    /// A detached vector was neither attached nor retired.
    OutstandingOwner,
}

/// Owns all allocations, including detached ones, independently of ticket Drop.
#[derive(Default)]
pub struct Host {
    counter: u64,
    rooted: Vec<u64>,
    detached: Option<Vec<u64>>,
    poisoned: bool,
}

impl Host {
    /// Runs one exclusive frame. An ordinary returned Err does not roll back.
    ///
    /// Panics propagate and leave the host poisoned. There is deliberately no
    /// reset-poison method: this model cannot establish snapshot restoration.
    ///
    /// # Panics
    /// Propagates a panic from the callback.
    ///
    /// A detached ticket cannot escape through the result:
    /// ```compile_fail
    /// use ic_storage_native_feasibility::Host;
    /// let mut host = Host::default();
    /// let ticket = host.with_update(|mut frame| frame.detach().unwrap());
    /// ```
    /// Nor through an outer capture:
    /// ```compile_fail
    /// use ic_storage_native_feasibility::Host;
    /// let mut host = Host::default();
    /// let mut escaped = None;
    /// host.with_update(|mut frame| escaped = Some(frame.detach().unwrap()));
    /// ```
    /// Futures cannot smuggle tickets out of the frame:
    /// ```compile_fail
    /// use ic_storage_native_feasibility::Host;
    /// let mut host = Host::default();
    /// let future = host.with_update(|mut frame| {
    ///     let ticket = frame.detach().unwrap();
    ///     async move { drop(ticket); }
    /// });
    /// ```
    /// Tickets from different frames cannot be exchanged:
    /// ```compile_fail
    /// use ic_storage_native_feasibility::Host;
    /// let (mut a, mut b) = (Host::default(), Host::default());
    /// a.with_update(|mut outer| {
    ///     let ticket = outer.detach().unwrap();
    ///     b.with_update(|mut inner| inner.attach(ticket));
    /// });
    /// ```
    pub fn with_update<R>(
        &mut self,
        operation: impl for<'scope> FnOnce(Frame<'scope>) -> R,
    ) -> Result<R, Fault> {
        if self.poisoned {
            return Err(Fault::Poisoned);
        }
        // Set before user code, rather than relying on a guard's destructor.
        self.poisoned = true;
        let result = operation(Frame { host: self });
        if self.detached.is_some() {
            return Err(Fault::OutstandingOwner);
        }
        self.poisoned = false;
        Ok(result)
    }
}

/// Exclusive view with operations and scalar slices, never &mut Vec or &mut Host.
///
/// Taking or replacing an owning vector through a slice is rejected:
/// ```compile_fail
/// use ic_storage_native_feasibility::Host;
/// Host::default().with_update(|frame| {
///     let _: Vec<u64> = std::mem::take(frame.values().unwrap());
/// });
/// ```
/// ```compile_fail
/// use ic_storage_native_feasibility::Host;
/// Host::default().with_update(|frame| {
///     let _ = std::mem::replace(frame.values().unwrap(), Vec::<u64>::new());
/// });
/// ```
/// There is no consuming iterator over the owning carrier:
/// ```compile_fail
/// use ic_storage_native_feasibility::Host;
/// Host::default().with_update(|frame| frame.into_iter());
/// ```
/// Borrowed elements cannot escape either:
/// ```compile_fail
/// use ic_storage_native_feasibility::Host;
/// let mut host = Host::default();
/// let escaped = host.with_update(|frame| frame.values().unwrap());
/// ```
pub struct Frame<'scope> {
    host: &'scope mut Host,
}

impl<'scope> Frame<'scope> {
    /// Reads the record's non-owning integer field.
    #[must_use]
    pub fn counter(&self) -> u64 {
        self.host.counter
    }

    /// Updates the integer without offering access to the owning record.
    pub fn set_counter(&mut self, value: u64) {
        self.host.counter = value;
    }

    /// Borrows initialized scalar elements, excluding spare capacity and owners.
    pub fn values(&self) -> Result<&[u64], Refusal> {
        self.require_attached()?;
        Ok(&self.host.rooted)
    }

    /// Appends one scalar; refusal preserves the vector's logical state.
    pub fn push(&mut self, value: u64) -> Result<(), Refusal> {
        self.require_attached()?;
        if self.host.rooted.len() == MAX_VALUES {
            return Err(Refusal::Capacity);
        }
        self.host
            .rooted
            .try_reserve(1)
            .map_err(|_| Refusal::Allocation)?;
        self.host.rooted.push(value);
        Ok(())
    }

    /// Removes one scalar; the owning allocation never leaves the host.
    pub fn pop(&mut self) -> Result<Option<u64>, Refusal> {
        self.require_attached()?;
        Ok(self.host.rooted.pop())
    }

    /// Detaches into a host-owned slot. The ticket carries authority, not a Vec.
    pub fn detach(&mut self) -> Result<Ticket<'scope>, Refusal> {
        self.require_attached()?;
        self.host.detached = Some(std::mem::take(&mut self.host.rooted));
        Ok(Ticket { scope: PhantomData })
    }

    /// Consumes the ticket and restores the same allocation to its root.
    ///
    /// # Panics
    /// An internal invariant failure panics, leaving the host poisoned.
    pub fn attach(&mut self, _ticket: Ticket<'scope>) {
        self.host.rooted = self
            .host
            .detached
            .take()
            .expect("ticket requires detached owner");
    }

    /// Consumes the ticket, destroys the live vector and leaves an empty root.
    /// This does not claim allocator reuse or physical page reclamation.
    ///
    /// # Panics
    /// An internal invariant failure panics, leaving the host poisoned.
    pub fn retire(&mut self, _ticket: Ticket<'scope>) {
        drop(
            self.host
                .detached
                .take()
                .expect("ticket requires detached owner"),
        );
    }

    fn require_attached(&self) -> Result<(), Refusal> {
        if self.host.detached.is_some() {
            Err(Refusal::Detached)
        } else {
            Ok(())
        }
    }
}

/// Invariant frame-bound authority; dropping it does not settle the obligation.
///
/// Tickets cannot clone an owner or duplicate attachment authority:
/// ```compile_fail
/// use ic_storage_native_feasibility::Host;
/// Host::default().with_update(|mut frame| {
///     let ticket = frame.detach().unwrap();
///     let duplicate = ticket.clone();
/// });
/// ```
#[must_use = "attach or retire the owner before the frame completes"]
pub struct Ticket<'scope> {
    scope: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

#[cfg(test)]
mod tests;
