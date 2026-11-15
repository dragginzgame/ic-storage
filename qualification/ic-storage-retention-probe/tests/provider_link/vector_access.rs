//! Instrument the bounded vector diagnostic; guard imports remain unresolved.
//! This transformation is not source identity, Rust validity or final admission.

use ic_host_artifacts::wasm;
use wasm_encoder::{
    CodeSection, EntityType, ExportSection, FunctionSection, GlobalSection, ImportSection,
    Instruction as I, MemorySection, Module, TypeSection, ValType, reencode::Reencode,
};
use wasmparser::{ExternalKind, FuncType, Operator as Op, Parser, Payload, TypeRef};

pub(super) const HELPERS: [&str; 7] = [
    "load_i32",
    "load_u8",
    "load_i64",
    "store_i32",
    "store_u8",
    "store_i64",
    "copy",
];

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Malformed,
    Profile,
    Stack,
    Encoding,
}

type Result<T> = std::result::Result<T, Error>;

fn checked<T, E>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|_| Error::Encoding)
}

fn inspect(raw: &[u8]) -> Result<()> {
    wasm::validate(raw, super::artifact::LIMIT).map_err(|_| Error::Malformed)?;
    let mut types = Vec::new();
    let mut imports = Vec::new();
    let mut functions = Vec::new();
    let mut exports = Vec::new();
    let mut globals = 0;
    let mut memories = 0;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(raw) {
        match checked(payload)? {
            Payload::TypeSection(r) => {
                for ty in r.into_iter_err_on_gc_types() {
                    types.push(checked(ty)?);
                }
            }
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = checked(import)?;
                    imports.push((import.module, import.name, import.ty));
                }
            }
            Payload::FunctionSection(r) => {
                for index in r {
                    functions.push(checked(index)?);
                }
            }
            Payload::MemorySection(r) => {
                for memory in r {
                    let memory = checked(memory)?;
                    if memory.initial != 3
                        || memory.maximum != Some(3)
                        || memory.memory64
                        || memory.shared
                        || memory.page_size_log2.is_some()
                    {
                        return Err(Error::Profile);
                    }
                    memories += 1;
                }
            }
            Payload::GlobalSection(r) => {
                for global in r {
                    let global = checked(global)?;
                    if global.ty.content_type != wasmparser::ValType::I32
                        || global.ty.mutable != (globals == 0)
                        || global.ty.shared
                    {
                        return Err(Error::Profile);
                    }
                    let mut ops = global.init_expr.get_operators_reader();
                    if !matches!(checked(ops.read())?, Op::I32Const { value: 32768 })
                        || !matches!(checked(ops.read())?, Op::End)
                        || !ops.eof()
                    {
                        return Err(Error::Profile);
                    }
                    globals += 1;
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = checked(export)?;
                    exports.push((export.name, export.kind, export.index));
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => return Err(Error::Profile),
        }
    }
    use wasmparser::ValType::{I32, I64};
    let signature = |index: u32| types.get(index as usize);
    if memories != 1
        || globals != 3
        || bodies.len() != 2
        || functions.len() != 2
        || imports.len() != 1
        || imports[0].0 != "native_vector_diagnostic"
        || imports[0].1 != "claim_region"
    {
        return Err(Error::Profile);
    }
    let TypeRef::Func(import_type) = imports[0].2 else {
        return Err(Error::Profile);
    };
    if signature(import_type) != Some(&FuncType::new([], [I32]))
        || signature(functions[0]) != Some(&FuncType::new([I32, I32, I32], []))
        || signature(functions[1]) != Some(&FuncType::new([I32, I32, I32, I64], [I64]))
    {
        return Err(Error::Profile);
    }
    exports.sort_by_key(|export| export.0);
    if exports
        != [
            ("__data_end", ExternalKind::Global, 1),
            ("__heap_base", ExternalKind::Global, 2),
            ("__stack_pointer", ExternalKind::Global, 0),
            ("memory", ExternalKind::Memory, 0),
            ("provider_vector_step", ExternalKind::Func, 2),
        ]
    {
        return Err(Error::Profile);
    }
    // This closed operator profile refuses unhandled memory operations before
    // rewriting. A new opcode must qualify both stack/control and access handling.
    let report = super::vector_stack::inspect(&bodies, 2, 0, 32768).map_err(|_| Error::Stack)?;
    if report.frame_bytes != 64 || report.lowest_stack_pointer != 32704 {
        return Err(Error::Stack);
    }
    Ok(())
}

struct Mapping;

impl Reencode for Mapping {
    type Error = std::convert::Infallible;

    fn function_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(if index == 0 {
            0
        } else {
            index + HELPERS.len() as u32
        })
    }
}

pub(super) fn prepare(raw: &[u8]) -> Result<Vec<u8>> {
    inspect(raw)?;
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut memory = MemorySection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut mapping = Mapping;
    let mut helper_types = 0;
    for payload in Parser::new(0).parse_all(raw) {
        match checked(payload)? {
            Payload::TypeSection(r) => {
                checked(mapping.parse_type_section(&mut types, r))?;
                helper_types = types.len();
                use ValType::{I32, I64};
                for (parameters, results) in [
                    (vec![I32, I64], vec![I32]),
                    (vec![I32, I64], vec![I32]),
                    (vec![I32, I64], vec![I64]),
                    (vec![I32, I32, I64], vec![]),
                    (vec![I32, I32, I64], vec![]),
                    (vec![I32, I64, I64], vec![]),
                    (vec![I32, I32, I32], vec![]),
                ] {
                    types.ty().function(parameters, results);
                }
            }
            Payload::ImportSection(r) => {
                checked(mapping.parse_import_section(&mut imports, r))?;
                for (index, name) in HELPERS.iter().enumerate() {
                    imports.import(
                        "native_vector_access",
                        name,
                        EntityType::Function(helper_types + index as u32),
                    );
                }
            }
            Payload::FunctionSection(r) => {
                checked(mapping.parse_function_section(&mut functions, r))?
            }
            Payload::MemorySection(r) => checked(mapping.parse_memory_section(&mut memory, r))?,
            Payload::GlobalSection(r) => checked(mapping.parse_global_section(&mut globals, r))?,
            Payload::ExportSection(r) => checked(mapping.parse_export_section(&mut exports, r))?,
            Payload::CodeSectionEntry(body) => {
                let mut function = checked(mapping.new_function_with_parsed_locals(&body))?;
                for op in checked(body.get_operators_reader())? {
                    let op = checked(op)?;
                    let access = match &op {
                        Op::I32Load { memarg } => Some((1, memarg)),
                        Op::I32Load8U { memarg } => Some((2, memarg)),
                        Op::I64Load { memarg } => Some((3, memarg)),
                        Op::I32Store { memarg } => Some((4, memarg)),
                        Op::I32Store8 { memarg } => Some((5, memarg)),
                        Op::I64Store { memarg } => Some((6, memarg)),
                        _ => None,
                    };
                    if let Some((helper, memarg)) = access {
                        if memarg.target != wasmparser::MemArgTarget::Memory(0)
                            || memarg.ordering.is_some()
                        {
                            return Err(Error::Profile);
                        }
                        // Preserve the full unsigned memory immediate. Existing
                        // stack operands remain address/value in their original order.
                        function.instruction(&I::I64Const(memarg.offset as i64));
                        function.instruction(&I::Call(helper));
                    } else if matches!(op, Op::MemoryCopy { .. }) {
                        function.instruction(&I::Call(7));
                    } else if matches!(
                        op,
                        Op::Block { .. }
                            | Op::End
                            | Op::Br { .. }
                            | Op::BrIf { .. }
                            | Op::BrTable { .. }
                            | Op::Return
                            | Op::Unreachable
                            | Op::Call { .. }
                            | Op::GlobalGet { .. }
                            | Op::GlobalSet { .. }
                            | Op::LocalGet { .. }
                            | Op::LocalSet { .. }
                            | Op::LocalTee { .. }
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
                    ) {
                        function.instruction(&checked(mapping.instruction(op))?);
                    } else {
                        return Err(Error::Profile);
                    }
                }
                code.function(&function);
            }
            Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => return Err(Error::Profile),
        }
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&memory)
        .section(&globals)
        .section(&exports)
        .section(&code);
    let bytes = module.finish();
    wasm::validate(&bytes, super::artifact::LIMIT).map_err(|_| Error::Encoding)?;
    Ok(bytes)
}

#[cfg(test)]
pub(super) mod tests;
