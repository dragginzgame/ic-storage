//! Partial vector proof: bounded, restored transient stack and acyclic direct calls.
//! This does not admit payload addresses, provider custody or installed execution.

use wasmparser::{BlockType, FunctionBody, Operator as Op};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Shape,
    Stack,
    Control,
    Call,
    Cycle,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Report {
    /// Maximum simultaneous frame sum along a reachable direct call path.
    pub(super) frame_bytes: u32,
    pub(super) lowest_stack_pointer: u32,
    pub(super) direct_edges: usize,
}

/// Caller first validates Wasm, the sole non-reentrant foreign import, global
/// roster/initializer and entrypoint identity. Every stack-using body must have
/// one balanced frame. Acyclic direct calls bound the sum of simultaneously live
/// frames; no callee may change its caller's frame local or return unbalanced.
pub(super) fn inspect(
    bodies: &[FunctionBody<'_>],
    entry: u32,
    stack_global: u32,
    stack_top: u32,
) -> Result<Report, Error> {
    inspect_profile(bodies, entry, stack_global, stack_top, false, 1)
}

pub(super) fn inspect_map(
    bodies: &[FunctionBody<'_>],
    entry: u32,
    stack_global: u32,
    stack_top: u32,
) -> Result<Report, Error> {
    inspect_profile(bodies, entry, stack_global, stack_top, true, 1)
}

/// Current map artifact: claim plus a scalar-only, non-reentrant staging leaf.
pub(super) fn inspect_staged_map(
    bodies: &[FunctionBody<'_>],
    entry: u32,
    stack_global: u32,
    stack_top: u32,
) -> Result<Report, Error> {
    inspect_profile(bodies, entry, stack_global, stack_top, true, 2)
}

// Map-only extension: loops cannot change the one immutable frame local and
// the call graph remains acyclic. This bounds stack space, not execution time.
// Scalar arithmetic has no memory effects; fill/load32 get explicit guards.
pub(super) fn map_operator(op: &Op<'_>) -> bool {
    matches!(
        op,
        Op::Loop {
            blockty: BlockType::Empty
        } | Op::I32Mul
            | Op::I32Ctz
            | Op::I32DivU
            | Op::I32Or
            | Op::I32Popcnt
            | Op::I32RemU
            | Op::I32ShrU
            | Op::I32Xor
            | Op::I64And
            | Op::I64Eq
            | Op::I64Eqz
            | Op::I64Mul
            | Op::I64Ne
            | Op::I64LeU
            | Op::I64ShrU
            | Op::I64Xor
    )
}

fn inspect_profile(
    bodies: &[FunctionBody<'_>],
    entry: u32,
    stack_global: u32,
    stack_top: u32,
    map: bool,
    imports: usize,
) -> Result<Report, Error> {
    if !stack_top.is_multiple_of(16) {
        return Err(Error::Stack);
    }
    if bodies.is_empty()
        || bodies.len() > 16
        || (entry as usize) < imports
        || entry as usize >= bodies.len() + imports
    {
        return Err(Error::Shape);
    }
    let mut edges = vec![Vec::new(); bodies.len()];
    let mut frames = vec![0; bodies.len()];
    for (index, body) in bodies.iter().enumerate() {
        let ops = body
            .get_operators_reader()
            .map_err(|_| Error::Shape)?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::Shape)?;
        let frame = if ops
            .iter()
            .any(|op| matches!(op, Op::GlobalGet { .. } | Op::GlobalSet { .. }))
        {
            let frame = frame(&ops, stack_global, map)?;
            frames[index] = frame.bytes;
            Some(frame)
        } else {
            None
        };
        let mut depth = 0_u32;
        for (position, op) in ops.iter().enumerate() {
            match op {
                Op::Loop {
                    blockty: BlockType::Empty,
                } if map => depth += 1,
                Op::Block {
                    blockty: BlockType::Empty,
                } => depth += 1,
                Op::End if depth > 0 => depth -= 1,
                Op::End if position + 1 == ops.len() => {}
                Op::End => return Err(Error::Control),
                Op::Br { relative_depth } | Op::BrIf { relative_depth } => {
                    // The implicit function label would bypass stack restoration.
                    if *relative_depth >= depth {
                        return Err(Error::Control);
                    }
                }
                Op::BrTable { targets } => {
                    if targets.default() >= depth {
                        return Err(Error::Control);
                    }
                    for target in targets.targets() {
                        if target.map_err(|_| Error::Control)? >= depth {
                            return Err(Error::Control);
                        }
                    }
                }
                Op::Return => {
                    if frame
                        .as_ref()
                        .is_some_and(|f| position != f.exit_at || depth != 1)
                    {
                        return Err(Error::Control);
                    }
                }
                Op::GlobalGet { global_index } => {
                    if frame.is_none() || position != 0 || *global_index != stack_global {
                        return Err(Error::Stack);
                    }
                }
                Op::GlobalSet { global_index } => {
                    let Some(frame) = &frame else {
                        return Err(Error::Stack);
                    };
                    if *global_index != stack_global
                        || (position != 4 && position != frame.restore_at)
                    {
                        return Err(Error::Stack);
                    }
                }
                Op::LocalSet { local_index } | Op::LocalTee { local_index } => {
                    if frame
                        .as_ref()
                        .is_some_and(|f| *local_index == f.local && position != 3)
                    {
                        return Err(Error::Stack);
                    }
                }
                Op::Call { function_index } => {
                    if *function_index as usize >= bodies.len() + imports {
                        return Err(Error::Call);
                    }
                    // Admitted foreign callbacks are non-reentrant leaves under
                    // the link contract; indirect and tail calls still refuse.
                    if *function_index as usize >= imports {
                        edges[index].push(*function_index as usize - imports);
                    }
                }
                Op::I64Load32U { .. } | Op::MemoryFill { mem: 0 } if map => {}
                op if map && map_operator(op) => {}
                Op::Unreachable
                | Op::LocalGet { .. }
                | Op::I32Const { .. }
                | Op::I64Const { .. }
                | Op::I32Add
                | Op::I32Sub
                | Op::I32And
                | Op::I32Shl
                | Op::I32Eq
                | Op::I32Eqz
                | Op::I32Ne
                | Op::I32GeU
                | Op::I32GtU
                | Op::I32LeU
                | Op::I32LtU
                | Op::I32WrapI64
                | Op::Select
                | Op::I64Add
                | Op::I64ExtendI32U
                | Op::I64Or
                | Op::I64Shl
                | Op::I64GeU
                | Op::I64GtU
                | Op::I64LtU
                | Op::I32Load { .. }
                | Op::I32Load8U { .. }
                | Op::I64Load { .. }
                | Op::I32Store { .. }
                | Op::I32Store8 { .. }
                | Op::I64Store { .. }
                | Op::MemoryCopy {
                    dst_mem: 0,
                    src_mem: 0,
                } => {}
                // Includes loops, indirect/tail calls, exceptions, growth and
                // future control operators. Unknown compiler shapes need review.
                _ => return Err(Error::Unsupported),
            }
        }
        if depth != 0 {
            return Err(Error::Control);
        }
    }
    let mut colors = vec![0; bodies.len()];
    for index in 0..bodies.len() {
        visit(index, &edges, &mut colors)?;
    }
    let frame_bytes = peak(
        entry as usize - imports,
        &edges,
        &frames,
        &mut vec![None; bodies.len()],
    )?;
    let lowest_stack_pointer = stack_top
        .checked_sub(frame_bytes)
        .filter(|floor| *floor > 0)
        .ok_or(Error::Stack)?;
    Ok(Report {
        frame_bytes,
        lowest_stack_pointer,
        direct_edges: edges.iter().map(Vec::len).sum(),
    })
}

// Called only after cycle rejection; conservatively count all direct call paths.
fn peak(
    index: usize,
    edges: &[Vec<usize>],
    frames: &[u32],
    memo: &mut [Option<u32>],
) -> Result<u32, Error> {
    if let Some(value) = memo[index] {
        return Ok(value);
    }
    let mut children = 0;
    for &next in &edges[index] {
        children = children.max(peak(next, edges, frames, memo)?);
    }
    let value = frames[index].checked_add(children).ok_or(Error::Stack)?;
    memo[index] = Some(value);
    Ok(value)
}

fn visit(index: usize, edges: &[Vec<usize>], colors: &mut [u8]) -> Result<(), Error> {
    match colors[index] {
        1 => return Err(Error::Cycle),
        2 => return Ok(()),
        _ => {}
    }
    colors[index] = 1;
    for &next in &edges[index] {
        visit(next, edges, colors)?;
    }
    colors[index] = 2;
    Ok(())
}

struct Frame {
    bytes: u32,
    local: u32,
    restore_at: usize,
    exit_at: usize,
}

fn frame(ops: &[Op<'_>], stack: u32, map: bool) -> Result<Frame, Error> {
    let [
        Op::GlobalGet { global_index },
        Op::I32Const { value: bytes },
        Op::I32Sub,
        Op::LocalTee { local_index: local },
        Op::GlobalSet { global_index: set },
        ..,
    ] = ops
    else {
        return Err(Error::Stack);
    };
    if *global_index != stack || *set != stack || *bytes <= 0 || *bytes % 16 != 0 {
        return Err(Error::Stack);
    }
    let restore_at = ops
        .iter()
        .rposition(|op| matches!(op, Op::GlobalSet { .. }))
        .ok_or(Error::Stack)?;
    if restore_at < 8 {
        return Err(Error::Stack);
    }
    if !matches!(&ops[restore_at - 3..=restore_at],
        [Op::LocalGet { local_index }, Op::I32Const { value }, Op::I32Add, Op::GlobalSet { global_index }]
        if local_index == local && value == bytes && *global_index == stack)
    {
        return Err(Error::Stack);
    }
    let explicit = ops.iter().position(|op| matches!(op, Op::Return));
    let exit_at = explicit.unwrap_or(ops.len() - 1);
    if exit_at <= restore_at || !matches!(ops.last(), Some(Op::End)) {
        return Err(Error::Control);
    }
    if explicit.is_some() && !matches!(&ops[exit_at + 1..], [Op::End, Op::Unreachable, Op::End]) {
        return Err(Error::Control);
    }
    // No branch target, callback or effect may enter or interrupt this suffix.
    // Return is dominated by restoration; all other exits trap after the outer
    // block. Local block branches cannot target the function's implicit label.
    for op in &ops[restore_at + 1..exit_at] {
        if !matches!(
            op,
            Op::LocalGet { .. }
                | Op::I32Const { .. }
                | Op::I64Const { .. }
                | Op::I64ExtendI32U
                | Op::I64Or
                | Op::I64Shl
        ) && !(map && matches!(op, Op::I32Shl | Op::I32Or | Op::I32Ctz))
        {
            return Err(Error::Control);
        }
    }
    Ok(Frame {
        bytes: *bytes as u32,
        local: *local,
        restore_at,
        exit_at,
    })
}

#[cfg(test)]
mod tests;
