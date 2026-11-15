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
    pub(super) frame_bytes: u32,
    pub(super) lowest_stack_pointer: u32,
    pub(super) direct_edges: usize,
}

/// Caller first validates Wasm, the sole non-reentrant foreign import, global
/// roster/initializer and entrypoint identity. Only the entry may use the stack.
pub(super) fn inspect(
    bodies: &[FunctionBody<'_>],
    entry: u32,
    stack_global: u32,
    stack_top: u32,
) -> Result<Report, Error> {
    if !stack_top.is_multiple_of(16) {
        return Err(Error::Stack);
    }
    if bodies.is_empty() || bodies.len() > 16 || entry == 0 || entry as usize > bodies.len() {
        return Err(Error::Shape);
    }
    let mut edges = vec![Vec::new(); bodies.len()];
    let mut frame_bytes = 0;
    for (index, body) in bodies.iter().enumerate() {
        let ops = body
            .get_operators_reader()
            .map_err(|_| Error::Shape)?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| Error::Shape)?;
        let frame = if index as u32 + 1 == entry {
            let frame = frame(&ops, stack_global)?;
            frame_bytes = frame.bytes;
            Some(frame)
        } else {
            None
        };
        let mut depth = 0_u32;
        for (position, op) in ops.iter().enumerate() {
            match op {
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
                        .is_some_and(|f| position != f.return_at || depth != 1)
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
                    if *function_index as usize > bodies.len() {
                        return Err(Error::Call);
                    }
                    // The one foreign provider is a non-reentrant leaf under the
                    // future link contract; no indirect or tail calls are accepted.
                    if *function_index != 0 {
                        edges[index].push(*function_index as usize - 1);
                    }
                }
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
                | Op::I64Add
                | Op::I64ExtendI32U
                | Op::I64Or
                | Op::I64Shl
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
    let lowest_stack_pointer = stack_top
        .checked_sub(frame_bytes)
        .filter(|floor| *floor > 0)
        .ok_or(Error::Stack)?;
    let mut colors = vec![0; bodies.len()];
    for index in 0..bodies.len() {
        visit(index, &edges, &mut colors)?;
    }
    Ok(Report {
        frame_bytes,
        lowest_stack_pointer,
        direct_edges: edges.iter().map(Vec::len).sum(),
    })
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
    return_at: usize,
}

fn frame(ops: &[Op<'_>], stack: u32) -> Result<Frame, Error> {
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
    let return_at = ops
        .iter()
        .position(|op| matches!(op, Op::Return))
        .ok_or(Error::Control)?;
    if return_at <= restore_at
        || !matches!(&ops[return_at + 1..], [Op::End, Op::Unreachable, Op::End])
    {
        return Err(Error::Control);
    }
    // No branch target, callback or effect may enter or interrupt this suffix.
    // Return is dominated by restoration; all other exits trap after the outer
    // block. Local block branches cannot target the function's implicit label.
    for op in &ops[restore_at + 1..return_at] {
        if !matches!(
            op,
            Op::LocalGet { .. }
                | Op::I32Const { .. }
                | Op::I64Const { .. }
                | Op::I64ExtendI32U
                | Op::I64Or
                | Op::I64Shl
        ) {
            return Err(Error::Control);
        }
    }
    Ok(Frame {
        bytes: *bytes as u32,
        local: *local,
        restore_at,
        return_at,
    })
}

#[cfg(test)]
mod tests;
