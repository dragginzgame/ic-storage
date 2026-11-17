//! Instrument the bounded vector diagnostic; guard imports remain unresolved.
//! This transformation is not source identity, Rust validity or final admission.

use ic_storage_retention_probe::artifact::validate_wasm;
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

pub(super) const MAP_HELPERS: [&str; 8] = [
    "load_i32",
    "load_u8",
    "load_i64",
    "store_i32",
    "store_u8",
    "store_i64",
    "copy",
    "fill",
];

/// Current pinned compiler output; historical two-body fixtures remain test inputs.
pub(super) const NATIVE_FUNCTIONS: u32 = 8;

pub(super) struct Profile {
    pub(super) functions: u32,
    pub(super) entry: u32,
    pub(super) map: bool,
}

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

pub(super) fn inspect(raw: &[u8]) -> Result<Profile> {
    inspect_element(raw, 0)
}

pub(super) fn inspect_record(raw: &[u8]) -> Result<Profile> {
    inspect_element(raw, 3)
}

pub(super) fn inspect_record_v2(raw: &[u8]) -> Result<Profile> {
    inspect_element(raw, 2)
}

pub(super) fn inspect_map(raw: &[u8]) -> Result<Profile> {
    inspect_element(raw, 4)
}

// Closed profiles, not caller-configurable layout admission: 0 selects u64,
// 2 and 3 select concrete record schemas; 4 selects the scalar-map compiler shape.
fn inspect_element(raw: &[u8], record: u32) -> Result<Profile> {
    validate_wasm(raw, super::artifact::LIMIT).map_err(|_| Error::Malformed)?;
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
        || bodies.len() != functions.len()
        || if record == 4 {
            functions.len() != 9
        } else if record == 2 {
            functions.len() != 6
        } else if record == 3 {
            functions.len() != 2
        } else {
            !matches!(functions.len(), 2 | 8)
        }
        || imports.len() != if record == 4 { 2 } else { 1 }
        || imports[0].0
            != if record == 4 {
                "native_map_diagnostic"
            } else if record == 2 {
                "native_record_vector_v2_diagnostic"
            } else if record == 3 {
                "native_record_vector_diagnostic"
            } else {
                "native_vector_diagnostic"
            }
        || imports[0].1 != "claim_region"
    {
        return Err(Error::Profile);
    }
    let TypeRef::Func(import_type) = imports[0].2 else {
        return Err(Error::Profile);
    };
    if record == 4 {
        let TypeRef::Func(staged_type) = imports[1].2 else {
            return Err(Error::Profile);
        };
        if imports[1].0 != "native_map_diagnostic"
            || imports[1].1 != "staged_word"
            || signature(staged_type) != Some(&FuncType::new([I32, I32], [I64]))
        {
            return Err(Error::Profile);
        }
    }
    let expected = if record == 4 {
        vec![
            FuncType::new([I32, I32], [I32]),
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32, I32, I64, I64], []),
            FuncType::new([I32, I32], []),
            FuncType::new([I32, I32, I64], []),
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32, I32, I32, I64, I64], [I64]),
            FuncType::new([I32, I32, I32], [I32]),
            FuncType::new([], []),
        ]
    } else if record == 2 {
        vec![
            FuncType::new([I32], []),
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32, I32], []),
            FuncType::new([I32, I32, I32, I64], [I64]),
            FuncType::new([], []),
            FuncType::new([], []),
        ]
    } else if functions.len() == 2 {
        vec![
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32, I32, I32, I64], [I64]),
        ]
    } else {
        vec![
            FuncType::new([I32, I32], [I32]),
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32], []),
            FuncType::new([I32, I32, I32], []),
            FuncType::new([I32, I32], []),
            FuncType::new([I32, I32, I32, I64], [I64]),
            FuncType::new([], []),
            FuncType::new([], []),
        ]
    };
    if signature(import_type) != Some(&FuncType::new([], [I32]))
        || functions
            .iter()
            .zip(&expected)
            .any(|(index, ty)| signature(*index) != Some(ty))
    {
        return Err(Error::Profile);
    }
    let (entry, peak) = if record == 4 {
        (8, 2688)
    } else if record == 2 {
        (4, 80)
    } else if functions.len() == 2 {
        (2, 64)
    } else {
        (6, 80)
    };
    exports.sort_by_key(|export| export.0);
    if exports
        != [
            ("__data_end", ExternalKind::Global, 1),
            ("__heap_base", ExternalKind::Global, 2),
            ("__stack_pointer", ExternalKind::Global, 0),
            ("memory", ExternalKind::Memory, 0),
            (
                if record == 4 {
                    "provider_map_step"
                } else if record != 0 {
                    "provider_record_vector_step"
                } else {
                    "provider_vector_step"
                },
                ExternalKind::Func,
                entry,
            ),
        ]
    {
        return Err(Error::Profile);
    }
    // This closed operator profile refuses unhandled memory operations before
    // rewriting. A new opcode must qualify both stack/control and access handling.
    let report = if record == 4 {
        super::vector_stack::inspect_staged_map(&bodies, entry, 0, 32768)
    } else {
        super::vector_stack::inspect(&bodies, entry, 0, 32768)
    }
    .map_err(|_| Error::Stack)?;
    if report.frame_bytes != peak || report.lowest_stack_pointer != 32768 - peak {
        return Err(Error::Stack);
    }
    Ok(Profile {
        functions: functions.len() as u32,
        entry,
        map: record == 4,
    })
}

struct Mapping {
    helpers: u32,
    imports: u32,
}

impl Reencode for Mapping {
    type Error = std::convert::Infallible;

    fn function_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(if index < self.imports {
            index
        } else {
            index + self.helpers
        })
    }
}

pub(super) fn prepare(raw: &[u8]) -> Result<Vec<u8>> {
    inspect(raw)?;
    rewrite_accesses(raw, false)
}

pub(super) fn prepare_record(raw: &[u8]) -> Result<Vec<u8>> {
    inspect_record(raw)?;
    rewrite_accesses(raw, false)
}

pub(super) fn prepare_record_v2(raw: &[u8]) -> Result<Vec<u8>> {
    inspect_record_v2(raw)?;
    rewrite_accesses(raw, false)
}

pub(super) fn prepare_map(raw: &[u8]) -> Result<Vec<u8>> {
    inspect_map(raw)?;
    rewrite_accesses(raw, true)
}

fn rewrite_accesses(raw: &[u8], map: bool) -> Result<Vec<u8>> {
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut memory = MemorySection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let helpers: &[&str] = if map { &MAP_HELPERS } else { &HELPERS };
    let mut mapping = Mapping {
        helpers: helpers.len() as u32,
        imports: if map { 2 } else { 1 },
    };
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
                if map {
                    types.ty().function([ValType::I32; 3], []);
                }
            }
            Payload::ImportSection(r) => {
                checked(mapping.parse_import_section(&mut imports, r))?;
                for (index, name) in helpers.iter().enumerate() {
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
                        Op::I64Load32U { memarg } if map => Some((1, memarg)),
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
                        function.instruction(&I::Call(helper + u32::from(map)));
                        if matches!(op, Op::I64Load32U { .. }) {
                            function.instruction(&I::I64ExtendI32U);
                        }
                    } else if map && matches!(op, Op::MemoryFill { mem: 0 }) {
                        function.instruction(&I::Call(9));
                    } else if matches!(op, Op::MemoryCopy { .. }) {
                        function.instruction(&I::Call(7 + u32::from(map)));
                    } else if map && super::vector_stack::map_operator(&op)
                        || matches!(
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
                        )
                    {
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
    validate_wasm(&bytes, super::artifact::LIMIT).map_err(|_| Error::Encoding)?;
    Ok(bytes)
}

#[cfg(test)]
pub(super) mod tests;
