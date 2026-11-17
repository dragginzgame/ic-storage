//! Deterministic artifact admission for the Rust 1.100.0 retained runtime.
//! Keeps only supplier-owned scalar directories/native windows. Compiler data,
//! allocator metadata and ordinary Global allocations are reconstructed on upgrade.
use std::{borrow::Cow, collections::BTreeMap, vec::Vec};
use wasm_encoder::{
    self as enc, Instruction as I,
    reencode::{self, Reencode},
};
use wasmparser::{self as parse, ExternalKind, Operator, Parser, Payload, TypeRef};

/// Linker start of compiler-owned static data, beyond both native banks.
pub const GLOBAL_BASE: u32 = 36 * 1024 * 1024;
/// Required initial linear memory: native windows plus transient application heap.
pub const INITIAL_MEMORY: u64 = 40 * 1024 * 1024;
/// Supported maximum linear memory, including ordinary allocator growth.
pub const MAXIMUM_MEMORY: u64 = 128 * 1024 * 1024;
/// Required stack-first stack reservation.
pub const STACK_BYTES: u32 = 1024 * 1024;
const HEADER: i32 = 1_572_864;
const IMPORT: &str = "ic_storage_retained_v1";
const MARKER: &str = "icp:private enhanced-orthogonal-persistence";
/// Artifact refusal before deployment. No input bytes are modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Malformed or unsupported Wasm.
    Wasm,
    /// Unapproved memory, imports, compiler placement or lifecycle exports.
    Contract,
    /// No unique mandatory runtime import, or already prepared startup.
    Startup,
    /// Prepared bytes differ from the deterministic admitted source transform.
    Identity,
}
struct Plan {
    gate: u32,
    gate_type: u32,
    total: u32,
    types: u32,
    segments: Vec<(i32, Vec<u8>)>,
}
fn inspect(raw: &[u8]) -> Result<Plan, Error> {
    if raw.len() > 32 * 1024 * 1024 {
        return Err(Error::Contract);
    }
    parse::Validator::new()
        .validate_all(raw)
        .map_err(|_| Error::Wasm)?;
    let (mut imports, mut defined, mut memories) = (0, 0, 0);
    let mut gate = None;
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut globals = Vec::new();
    let mut layouts = BTreeMap::new();
    let mut hooks = Vec::new();
    let mut segments = Vec::new();
    for item in Parser::new(0).parse_all(raw) {
        match item.map_err(|_| Error::Wasm)? {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    types.push(ty.map_err(|_| Error::Contract)?);
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(|_| Error::Wasm)?;
                    let TypeRef::Func(ty) = import.ty else {
                        return Err(Error::Contract);
                    };
                    if import.module == IMPORT && import.name == "admitted" {
                        if gate.replace((imports, ty)).is_some() {
                            return Err(Error::Startup);
                        }
                    } else if import.module != "ic0" {
                        return Err(Error::Contract);
                    }
                    imports += 1;
                    functions.push(ty);
                }
            }
            Payload::FunctionSection(reader) => {
                defined = reader.count();
                for ty in reader {
                    functions.push(ty.map_err(|_| Error::Wasm)?);
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|_| Error::Wasm)?;
                    if memory.memory64
                        || memory.shared
                        || memory.initial != INITIAL_MEMORY / 65536
                        || memory.maximum != Some(MAXIMUM_MEMORY / 65536)
                        || memory.page_size_log2.is_some()
                    {
                        return Err(Error::Contract);
                    }
                    memories += 1;
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.map_err(|_| Error::Wasm)?;
                    let mut ops = global.init_expr.get_operators_reader();
                    let Operator::I32Const { value } = ops.read().map_err(|_| Error::Wasm)? else {
                        return Err(Error::Contract);
                    };
                    if !matches!(ops.read().map_err(|_| Error::Wasm)?, Operator::End) || !ops.eof()
                    {
                        return Err(Error::Contract);
                    }
                    globals.push((value as u32, global.ty.mutable));
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|_| Error::Wasm)?;
                    if matches!(
                        export.name,
                        "__data_end" | "__heap_base" | "__stack_pointer"
                    ) {
                        if export.kind != ExternalKind::Global {
                            return Err(Error::Contract);
                        }
                        layouts.insert(
                            export.name,
                            *globals.get(export.index as usize).ok_or(Error::Contract)?,
                        );
                    }
                    if matches!(
                        export.name,
                        "canister_init" | "canister_pre_upgrade" | "canister_post_upgrade"
                    ) {
                        if export.kind != ExternalKind::Func {
                            return Err(Error::Contract);
                        }
                        let ty = functions
                            .get(export.index as usize)
                            .and_then(|i| types.get(*i as usize))
                            .ok_or(Error::Contract)?;
                        if !ty.params().is_empty() || !ty.results().is_empty() {
                            return Err(Error::Contract);
                        }
                        hooks.push(export.name);
                    }
                }
            }
            Payload::DataSection(reader) => {
                for segment in reader {
                    let segment = segment.map_err(|_| Error::Wasm)?;
                    let parse::DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = segment.kind
                    else {
                        return Err(Error::Contract);
                    };
                    let mut ops = offset_expr.get_operators_reader();
                    let Operator::I32Const { value } = ops.read().map_err(|_| Error::Wasm)? else {
                        return Err(Error::Contract);
                    };
                    if !matches!(ops.read().map_err(|_| Error::Wasm)?, Operator::End)
                        || !ops.eof()
                        || value < GLOBAL_BASE as i32
                        || (value as usize)
                            .checked_add(segment.data.len())
                            .is_none_or(|end| end > GLOBAL_BASE as usize + STACK_BYTES as usize)
                    {
                        return Err(Error::Contract);
                    }
                    segments.push((value, segment.data.to_vec()));
                }
            }
            Payload::StartSection { .. } | Payload::DataCountSection { .. } => {
                return Err(Error::Startup);
            }
            Payload::CustomSection(section) if section.name() == MARKER => {
                return Err(Error::Startup);
            }
            _ => {}
        }
    }
    let (gate, gate_type) = gate.ok_or(Error::Startup)?;
    let ty = types.get(gate_type as usize).ok_or(Error::Contract)?;
    if !ty.params().is_empty()
        || ty.results() != [parse::ValType::I32]
        || memories != 1
        || hooks.len() != 3
        || segments.is_empty()
        || layouts.get("__stack_pointer") != Some(&(STACK_BYTES, true))
    {
        return Err(Error::Contract);
    }
    let &(end, false) = layouts.get("__data_end").ok_or(Error::Contract)? else {
        return Err(Error::Contract);
    };
    let &(heap, false) = layouts.get("__heap_base").ok_or(Error::Contract)? else {
        return Err(Error::Contract);
    };
    if end < GLOBAL_BASE
        || end > heap
        || heap >= GLOBAL_BASE + STACK_BYTES
        || segments
            .iter()
            .any(|(offset, data)| *offset as usize + data.len() > end as usize)
    {
        return Err(Error::Contract);
    }
    Ok(Plan {
        gate,
        gate_type,
        total: imports + defined,
        types: types.len() as u32,
        segments,
    })
}
struct Transform {
    plan: Plan,
}
impl Reencode for Transform {
    type Error = core::convert::Infallible;
    fn function_index(&mut self, index: u32) -> Result<u32, reencode::Error<Self::Error>> {
        Ok(if index == self.plan.gate {
            self.plan.total - 1
        } else if index > self.plan.gate {
            index - 1
        } else {
            index
        })
    }
    fn parse_import_section(
        &mut self,
        output: &mut enc::ImportSection,
        reader: parse::ImportSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        for item in reader.into_imports() {
            let item = item?;
            if item.module != IMPORT {
                self.parse_import(output, item)?;
            }
        }
        Ok(())
    }
    fn parse_type_section(
        &mut self,
        output: &mut enc::TypeSection,
        reader: parse::TypeSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_type_section(self, output, reader)?;
        output.ty().function([], []);
        Ok(())
    }
    fn parse_function_section(
        &mut self,
        output: &mut enc::FunctionSection,
        reader: parse::FunctionSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_function_section(self, output, reader)?;
        output.function(self.plan.gate_type);
        output.function(self.plan.types);
        Ok(())
    }
    fn parse_data_section(
        &mut self,
        output: &mut enc::DataSection,
        _: parse::DataSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        for (_, data) in &self.plan.segments {
            output.passive(data.iter().copied());
        }
        Ok(())
    }
    fn parse_code_section(
        &mut self,
        output: &mut enc::CodeSection,
        reader: parse::CodeSectionReader<'_>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        reencode::utils::parse_code_section(self, output, reader)?;
        let mut gate = enc::Function::new([]);
        gate.instruction(&I::I32Const(0x535401));
        gate.instruction(&I::End);
        output.function(&gate);
        let mut start = enc::Function::new([]);
        // No Rust frame exists yet. Retained directory at HEADER and banks
        // [2MiB,34MiB) are untouched; all transient stack/compiler/Global storage
        // is cleared before compiler data is replayed, including grown heap pages.
        for op in [
            I::I32Const(0),
            I::I32Const(0),
            I::I32Const(HEADER),
            I::MemoryFill(0),
            I::I32Const(GLOBAL_BASE as i32),
            I::I32Const(0),
            I::MemorySize(0),
            I::I32Const(16),
            I::I32Shl,
            I::I32Const(GLOBAL_BASE as i32),
            I::I32Sub,
            I::MemoryFill(0),
        ] {
            start.instruction(&op);
        }
        for (index, (offset, data)) in self.plan.segments.iter().enumerate() {
            for op in [
                I::I32Const(*offset),
                I::I32Const(0),
                I::I32Const(data.len() as i32),
                I::MemoryInit {
                    mem: 0,
                    data_index: index as u32,
                },
            ] {
                start.instruction(&op);
            }
        }
        start.instruction(&I::End);
        output.function(&start);
        Ok(())
    }
    fn intersperse_section_hook(
        &mut self,
        module: &mut enc::Module,
        after: Option<enc::SectionId>,
        before: Option<enc::SectionId>,
    ) -> Result<(), reencode::Error<Self::Error>> {
        if after == Some(enc::SectionId::Export) {
            module.section(&enc::StartSection {
                function_index: self.plan.total,
            });
        }
        if before == Some(enc::SectionId::Code) {
            module.section(&enc::DataCountSection {
                count: self.plan.segments.len() as u32,
            });
        }
        Ok(())
    }
}
/// Prepare a linked Rust 1.100.0 application for supplier retained startup.
/// Requires stack-first 1-MiB stack, global base 36 MiB, initial/max memory
/// 40/128 MiB, exported linker globals and init/pre/post-upgrade hooks. The runtime
/// import is resolved only together with mandatory transient reset and data replay.
/// Native representations and application lifecycle obligations remain explicit;
/// this does not audit arbitrary unsafe application code or certify other compilers.
///
/// # Errors
/// Refuses malformed, already prepared or differently laid-out artifacts.
pub fn prepare(raw: &[u8]) -> Result<Vec<u8>, Error> {
    let plan = inspect(raw)?;
    let mut module = enc::Module::new();
    Transform { plan }
        .parse_core_module(&mut module, Parser::new(0), raw)
        .map_err(|_| Error::Wasm)?;
    module.section(&enc::CustomSection {
        name: Cow::Borrowed(MARKER),
        data: Cow::Borrowed(b"ic-storage-retained-v1"),
    });
    let bytes = module.finish();
    parse::Validator::new()
        .validate_all(&bytes)
        .map_err(|_| Error::Wasm)?;
    Ok(bytes)
}
/// Verify the exact prepared output against its reviewed raw input.
///
/// # Errors
/// Refuses invalid raw input or any changed prepared bytes.
pub fn admit(prepared: &[u8], raw: &[u8]) -> Result<(), Error> {
    if prepare(raw)? == prepared {
        Ok(())
    } else {
        Err(Error::Identity)
    }
}
