//! Explicit trusted fixture variants; no arbitrary WAT fragment is accepted.

#[derive(Clone, Copy, Debug)]
pub(crate) struct Profile {
    pub(crate) version: u64,
    pub(crate) root: (u64, u64),
    pub(crate) extent: u64,
    pub(crate) post_trap: bool,
    pub(crate) fault: Option<(u32, u64)>,
    pub(crate) source_guard: bool,
}

impl Profile {
    pub(crate) const A: Self = Self {
        version: 1,
        root: (3, 4),
        extent: 4096,
        post_trap: false,
        fault: None,
        source_guard: true,
    };
    pub(crate) const B: Self = Self {
        version: 2,
        ..Self::A
    };

    pub(super) fn check(self) -> super::Result<()> {
        if self.root.0 > self.root.1
            || self.root.1 > 512
            || !matches!(self.extent, 4096 | 8192)
            || self
                .fault
                .is_some_and(|(offset, _)| !(65536..=65584).contains(&offset) || offset % 8 != 0)
        {
            return Err("vector fixture profile");
        }
        Ok(())
    }
}
