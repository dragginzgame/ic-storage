use super::{Backing, Error, Rc, RefCell, State, Store, encode};

/// Non-cloneable participant grant. Its lifetime retains the backing owner.
pub struct Participant<B: Backing> {
    pub(super) state: Rc<RefCell<State<B>>>,
    pub(super) mask: u8,
}
/// Scoped cached reads of exactly the granted roots.
pub struct Read<'a, B: Backing> {
    state: &'a State<B>,
    mask: u8,
}
/// Scoped synchronous writes of exactly the granted roots.
pub struct Write<'a, B: Backing> {
    state: &'a mut State<B>,
    mask: u8,
}

impl<B: Backing> Participant<B> {
    /// Borrow admitted logical bytes without backing IO or durable mutation.
    ///
    /// # Errors
    /// Refuses an active mutable borrow or poisoned owner.
    pub fn read<R>(&self, operation: impl for<'a> FnOnce(Read<'a, B>) -> R) -> Result<R, Error> {
        read(&self.state, self.mask, operation)
    }
    /// Perform synchronous operations. A normal returned error preserves earlier
    /// writes; a host unwind quarantines every participant. IC traps rely on
    /// message rollback, including a late reply/encoding failure outside this call.
    /// Neither a view nor borrowed record may cross an await or escape the closure.
    ///
    /// ```compile_fail
    /// use ic_storage::records::{Backing, Participant};
    /// fn escape<B: Backing>(participant: &mut Participant<B>) {
    ///     let view = participant.update(|view| view).unwrap();
    /// }
    /// ```
    /// ```compile_fail
    /// use ic_storage::records::{Backing, Participant};
    /// fn escape<B: Backing>(participant: &Participant<B>) -> &[u8] {
    ///     participant.read(|view| view.get(1).unwrap()).unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// Refuses an active borrow or poisoned owner before invoking the callback.
    pub fn update<R>(
        &mut self,
        operation: impl for<'a> FnOnce(Write<'a, B>) -> R,
    ) -> Result<R, Error> {
        update(&self.state, self.mask, operation)
    }
}
impl<B: Backing> Store<B> {
    fn grants(&self, participants: &[&Participant<B>]) -> Result<u8, Error> {
        let mut mask = 0;
        for participant in participants {
            if !Rc::ptr_eq(&self.state, &participant.state) || mask & participant.mask != 0 {
                return Err(Error::Authority);
            }
            mask |= participant.mask;
        }
        if mask == 0 {
            return Err(Error::Authority);
        }
        Ok(mask)
    }
    /// Read across explicitly supplied grants from this same owner.
    ///
    /// # Errors
    /// Refuses foreign/duplicate/empty grants, active mutation or poison.
    pub fn read<R>(
        &self,
        participants: &[&Participant<B>],
        operation: impl for<'a> FnOnce(Read<'a, B>) -> R,
    ) -> Result<R, Error> {
        read(&self.state, self.grants(participants)?, operation)
    }
    /// Compose synchronous writes across supplied participant grants. This does not
    /// turn ordinary Rust errors into rollback or commit outgoing async callbacks.
    ///
    /// # Errors
    /// Refuses invalid grants, active borrows or poison before invoking the callback.
    pub fn update<R>(
        &mut self,
        participants: &[&Participant<B>],
        operation: impl for<'a> FnOnce(Write<'a, B>) -> R,
    ) -> Result<R, Error> {
        update(&self.state, self.grants(participants)?, operation)
    }
}
fn read<B: Backing, R>(
    state: &Rc<RefCell<State<B>>>,
    mask: u8,
    operation: impl for<'a> FnOnce(Read<'a, B>) -> R,
) -> Result<R, Error> {
    let state = state.try_borrow().map_err(|_| Error::Busy)?;
    if state.poisoned {
        return Err(Error::Poisoned);
    }
    Ok(operation(Read {
        state: &state,
        mask,
    }))
}
fn update<B: Backing, R>(
    state: &Rc<RefCell<State<B>>>,
    mask: u8,
    operation: impl for<'a> FnOnce(Write<'a, B>) -> R,
) -> Result<R, Error> {
    let mut state = state.try_borrow_mut().map_err(|_| Error::Busy)?;
    if state.poisoned {
        return Err(Error::Poisoned);
    }
    state.poisoned = true;
    let result = operation(Write {
        state: &mut state,
        mask,
    });
    state.poisoned = false;
    Ok(result)
}
fn index<B: Backing>(state: &State<B>, mask: u8, root: u64) -> Result<usize, Error> {
    state
        .slots
        .iter()
        .position(|slot| slot.root.id == root)
        .filter(|&index| mask & (1 << index) != 0)
        .ok_or(Error::Authority)
}
fn get<B: Backing>(state: &State<B>, mask: u8, root: u64) -> Result<&[u8], Error> {
    let bytes = state.slots[index(state, mask, root)?].record.get();
    Ok(&bytes[32..bytes.len() - 32])
}
impl<B: Backing> Read<'_, B> {
    /// Borrow exact logical bytes; no Storage envelope or mutable reference escapes.
    ///
    /// # Errors
    /// Refuses undeclared or ungranted roots.
    pub fn get(&self, root: u64) -> Result<&[u8], Error> {
        get(self.state, self.mask, root)
    }
}
impl<B: Backing> Write<'_, B> {
    /// Read the current value, including earlier writes in this scope.
    ///
    /// # Errors
    /// Refuses undeclared or ungranted roots.
    pub fn get(&self, root: u64) -> Result<&[u8], Error> {
        get(self.state, self.mask, root)
    }
    /// Replace a bounded record synchronously. Expected bound/allocation refusals
    /// precede this record's writes; earlier successful operations remain applied.
    /// Empty bytes are a real value, not an absent record or initialization signal.
    ///
    /// # Errors
    /// Refuses unauthorized roots, excessive values, allocation or backing errors.
    pub fn set(&mut self, root: u64, value: &[u8]) -> Result<(), Error> {
        let index = index(self.state, self.mask, root)?;
        let slot = &mut self.state.slots[index];
        let bytes = encode(slot.root, value)?;
        slot.record.set(bytes).map_err(Error::Backing)
    }
}
