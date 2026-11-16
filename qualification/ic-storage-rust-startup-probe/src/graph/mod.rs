//! Installed M0 fixture. No allocator, async execution or borrowed reply buffer.
//! All raw linear-memory custody is confined to `kernel`.

#[cfg(feature = "managed-bench")]
mod benchmark;
mod kernel;

use ic_platform_runtime::raw as ic0;
use ic_storage::m0::image::Error;

#[cfg(not(feature = "managed-bench"))]
pub(super) fn initialize() {
    kernel::initialize();
    if !kernel::grow() {
        std::arch::wasm32::unreachable();
    }
    kernel::update(|s| {
        let node = s.allocate_node(7, None)?;
        s.set_node_next(node, Some(node))?;
        let root = s.allocate_counter(41, Some(node), Some(node))?;
        s.publish_root(root)
    });
}

#[cfg(feature = "managed-bench")]
pub(super) fn initialize() {
    benchmark::initialize();
}

pub(super) fn admit() {
    kernel::admit();
}

#[expect(unsafe_code, reason = "unique fixture IC query export")]
#[unsafe(export_name = "canister_query graph")]
extern "C" fn graph() {
    let words = kernel::update(|s| {
        let root = s.root()?.ok_or(Error::Handle)?;
        let left = s.left(root)?.ok_or(Error::Handle)?;
        let right = s.right(root)?.ok_or(Error::Handle)?;
        let next = s.node_next(left)?.ok_or(Error::Handle)?;
        let summary = s.summary()?;
        Ok([
            summary.object_count as u64,
            summary.used_bytes,
            summary.next_identity,
            summary.committed_bytes,
            root.reference().offset,
            root.reference().identity,
            s.counter_value(root)?,
            s.node_value(left)?,
            s.node_value(right)?,
            left.reference().identity,
            right.reference().identity,
            next.reference().identity,
        ])
    });
    for value in words {
        ic0::msg_reply_data_append(&value.to_le_bytes());
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "unique fixture IC query export")]
#[unsafe(export_name = "canister_query graph_bytes")]
extern "C" fn graph_bytes() {
    // The test populations use at most three retained pages. Sending bounded
    // chunks avoids leaking a retained borrow into any callback or future.
    kernel::evidence();
    ic0::msg_reply();
}

fn change() {
    kernel::update(|s| {
        let root = s.root()?.ok_or(Error::Handle)?;
        let left = s.left(root)?.ok_or(Error::Handle)?;
        s.set_counter_value(root, 42)?;
        s.set_node_value(left, 99)?;
        s.allocate_node(8, Some(left))?;
        Ok(())
    });
}

#[expect(unsafe_code, reason = "unique fixture IC update export")]
#[unsafe(export_name = "canister_update graph_change")]
extern "C" fn graph_change() {
    change();
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "unique fixture IC update export")]
#[unsafe(export_name = "canister_update graph_grow")]
extern "C" fn graph_grow() {
    if !kernel::grow() {
        std::arch::wasm32::unreachable();
    }
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "unique fixture IC update export")]
#[unsafe(export_name = "canister_update graph_trap")]
extern "C" fn graph_trap() {
    // A resource refusal must not look like successful growth followed by the
    // deliberate trap. Return normally so the host's CanisterTrapped assertion
    // fails (or a negative upgrade succeeds and fails its rejection assertion).
    if !kernel::grow() {
        return;
    }
    change();
    std::arch::wasm32::unreachable();
}

#[expect(unsafe_code, reason = "unique fixture IC update export")]
#[unsafe(export_name = "canister_update graph_fill")]
extern "C" fn graph_fill() {
    kernel::update(|s| {
        // 160 + 907*72 = 65464; one more Node fits, the next crosses the page.
        while s.summary()?.object_count < 909 {
            s.allocate_node(0, None)?;
        }
        Ok(())
    });
    ic0::msg_reply();
}
