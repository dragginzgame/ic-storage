//! Exact two-function diagnostic profile, not a general linker or admission ABI.

use std::borrow::Cow;

use ic_host_artifacts::{wasm, wasm_text};
use wasm_encoder::{
    BlockType, CodeSection, CustomSection, ExportSection, Function, FunctionSection, GlobalSection,
    ImportSection, Instruction as I, MemArg, MemorySection, Module, TypeSection, ValType,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{ExternalKind, FuncType, Operator, Parser, Payload, TypeRef};

type Result<T> = std::result::Result<T, &'static str>;
pub(super) const LIMIT: usize = 65536;

#[derive(Clone, Copy, Debug)]
pub(super) enum Profile {
    A,
    B,
    Incompatible,
    Trapping,
}

impl Profile {
    pub(super) fn version(self) -> u64 {
        if matches!(self, Self::A) { 1 } else { 2 }
    }
}

fn checked<T, E>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|_| "parse or encode")
}

// Reviewed scalar bodies: obtain the base from the only foreign call, check
// null, and read/write only its first u64. No stack/global/allocator access.
pub(super) fn scalar_body(write: bool, claim: u32) -> Vec<u8> {
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&I::Block(BlockType::Empty));
    function.instruction(&I::Call(claim));
    function.instruction(&I::LocalTee(u32::from(write)));
    if write {
        function.instruction(&I::I32Eqz);
        function.instruction(&I::BrIf(0));
        function.instruction(&I::LocalGet(1));
        function.instruction(&I::LocalGet(0));
        function.instruction(&I::I64Store(MemArg {
            offset: 0,
            align: 3,
            target: wasm_encoder::MemArgTarget::Memory(0),
            ordering: None,
        }));
        function.instruction(&I::End);
    } else {
        function.instruction(&I::BrIf(0));
        function.instruction(&I::I64Const(0));
        function.instruction(&I::Return);
        function.instruction(&I::End);
        function.instruction(&I::LocalGet(0));
        function.instruction(&I::I64Load(MemArg {
            offset: 0,
            align: 3,
            target: wasm_encoder::MemArgTarget::Memory(0),
            ordering: None,
        }));
    }
    function.instruction(&I::End);
    function.into_raw_body()
}

pub(super) fn normalized(body: &wasmparser::FunctionBody<'_>) -> Result<Vec<u8>> {
    let mut copy = RoundtripReencoder;
    let mut function = checked(copy.new_function_with_parsed_locals(body))?;
    let mut reader = checked(body.get_operators_reader())?;
    while !reader.eof() {
        function.instruction(&checked(copy.parse_instruction(&mut reader))?);
    }
    Ok(function.into_raw_body())
}

pub(super) fn admit_raw(raw: &[u8]) -> Result<()> {
    wasm::validate(raw, LIMIT).map_err(|_| "malformed")?;
    let mut types = Vec::new();
    let mut imports = Vec::new();
    let mut functions = Vec::new();
    let mut exports = Vec::new();
    let mut memories = 0;
    let mut globals = 0;
    let mut bodies = 0;
    for payload in Parser::new(0).parse_all(raw) {
        match checked(payload)? {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    types.push(checked(ty)?);
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = checked(import)?;
                    imports.push((import.module, import.name, import.ty));
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    functions.push(checked(ty)?);
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = checked(memory)?;
                    memories += 1;
                    if memory.memory64
                        || memory.shared
                        || memory.initial != 3
                        || memory.maximum != Some(3)
                        || memory.page_size_log2.is_some()
                    {
                        return Err("memory");
                    }
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = checked(global)?;
                    let mut ops = global.init_expr.get_operators_reader();
                    if global.ty.content_type != wasmparser::ValType::I32
                        || global.ty.shared
                        || global.ty.mutable != (globals == 0)
                        || !matches!(checked(ops.read())?, Operator::I32Const { value: 65536 })
                        || !matches!(checked(ops.read())?, Operator::End)
                        || !ops.eof()
                    {
                        return Err("transient bounds");
                    }
                    globals += 1;
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = checked(export)?;
                    exports.push((export.name, export.kind, export.index));
                }
            }
            Payload::CodeSectionEntry(body) => {
                if bodies >= 2 || normalized(&body)? != scalar_body(bodies == 1, 0) {
                    return Err("scalar code");
                }
                bodies += 1;
            }
            Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => return Err("unsupported section"),
        }
    }
    use wasmparser::ValType::{I32, I64};
    if types
        != [
            FuncType::new([], [I32]),
            FuncType::new([], [I64]),
            FuncType::new([I64], []),
        ]
        || functions != [1, 2]
        || imports
            != [(
                "native_provider_diagnostic",
                "claim_region",
                TypeRef::Func(0),
            )]
        || memories != 1
        || globals != 3
        || bodies != 2
    {
        return Err("roster");
    }
    exports.sort_by_key(|entry| entry.0);
    if exports
        != [
            ("__data_end", ExternalKind::Global, 1),
            ("__heap_base", ExternalKind::Global, 2),
            ("__stack_pointer", ExternalKind::Global, 0),
            ("memory", ExternalKind::Memory, 0),
            ("provider_probe_read", ExternalKind::Func, 1),
            ("provider_probe_write", ExternalKind::Func, 2),
        ]
    {
        return Err("exports");
    }
    Ok(())
}

struct Mapping {
    claim: u32,
    functions: u32,
    types: u32,
    globals: u32,
}

impl Reencode for Mapping {
    type Error = std::convert::Infallible;
    fn function_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(if index == 0 {
            self.claim
        } else {
            self.functions + index - 1
        })
    }
    fn type_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.types + index)
    }
    fn global_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.globals + index)
    }
}

pub(super) fn prepare(raw: &[u8], profile: Profile) -> Result<Vec<u8>> {
    admit_raw(raw)?;
    // This temporary export resolves the named private claim, avoiding a guessed
    // function index. It is removed from the combined artifact's export roster.
    let source = include_str!("../provider_custody/provider.wat")
        .replace("__VERSION__", &profile.version().to_string())
        .replace(
            "__EXTENT__",
            if matches!(profile, Profile::Incompatible) {
                "8192"
            } else {
                "4096"
            },
        )
        .replace("__TRAP__", "0")
        .replace(
            "(func $claim (result i32)",
            "(func $claim (export \"__link_claim\") (result i32)",
        )
        .replace(
            "(func $settle\n",
            "(func $settle (export \"__link_settle\")\n",
        );
    let provider = wasm_text::compile(
        &source,
        wasm_text::CompileLimits {
            source_bytes: LIMIT,
            module_bytes: LIMIT,
        },
    )
    .map_err(|_| "provider source")?;
    wasm::validate(&provider, LIMIT).map_err(|_| "provider artifact")?;
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut memory = MemorySection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut custom = Vec::new();
    let mut claim = None;
    let mut settle = None;
    let mut post = None;
    let mut append = None;
    let mut reply = None;
    let mut copy = RoundtripReencoder;
    for payload in Parser::new(0).parse_all(&provider) {
        match checked(payload)? {
            Payload::TypeSection(r) => checked(copy.parse_type_section(&mut types, r))?,
            Payload::ImportSection(r) => {
                for (index, import) in r.clone().into_imports().enumerate() {
                    let import = checked(import)?;
                    if import.module != "ic0" || !matches!(import.ty, TypeRef::Func(_)) {
                        return Err("provider import");
                    }
                    let slot = match import.name {
                        "msg_reply_data_append" => &mut append,
                        "msg_reply" => &mut reply,
                        _ => return Err("provider import"),
                    };
                    if slot.replace(index as u32).is_some() {
                        return Err("duplicate import");
                    }
                }
                checked(copy.parse_import_section(&mut imports, r))?;
            }
            Payload::FunctionSection(r) => checked(copy.parse_function_section(&mut functions, r))?,
            Payload::MemorySection(r) => checked(copy.parse_memory_section(&mut memory, r))?,
            Payload::GlobalSection(r) => checked(copy.parse_global_section(&mut globals, r))?,
            Payload::ExportSection(r) => {
                for export in r {
                    let export = checked(export)?;
                    if matches!(
                        export.name,
                        "__link_claim" | "__link_settle" | "canister_post_upgrade"
                    ) {
                        if export.kind != ExternalKind::Func {
                            return Err("provider function");
                        }
                        let slot = match export.name {
                            "__link_claim" => &mut claim,
                            "__link_settle" => &mut settle,
                            _ => &mut post,
                        };
                        if slot.replace(export.index).is_some() {
                            return Err("duplicate provider function");
                        }
                    } else {
                        checked(copy.parse_export(&mut exports, export))?;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => checked(copy.parse_function_body(&mut code, body))?,
            Payload::CustomSection(section) => custom.push(CustomSection {
                name: Cow::Borrowed(section.name()),
                data: Cow::Borrowed(section.data()),
            }),
            Payload::Version { .. } | Payload::CodeSectionStart { .. } | Payload::End(_) => {}
            _ => return Err("provider section"),
        }
    }
    let mut mapping = Mapping {
        claim: claim.ok_or("missing claim")?,
        functions: imports.len() + functions.len(),
        types: types.len(),
        globals: globals.len(),
    };
    for payload in Parser::new(0).parse_all(raw) {
        match checked(payload)? {
            Payload::TypeSection(r) => checked(mapping.parse_type_section(&mut types, r))?,
            Payload::FunctionSection(r) => {
                checked(mapping.parse_function_section(&mut functions, r))?
            }
            Payload::GlobalSection(r) => checked(mapping.parse_global_section(&mut globals, r))?,
            Payload::ExportSection(r) => {
                for export in r {
                    let export = checked(export)?;
                    // Raw Rust functions remain private; only trusted lifecycle
                    // entrypoints may call them and then settle the region.
                    if export.kind == ExternalKind::Global {
                        checked(mapping.parse_export(&mut exports, export))?;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                checked(mapping.parse_function_body(&mut code, body))?
            }
            // The admitted memory is shared with the provider; the sole foreign
            // function import is replaced by its actual private implementation.
            _ => {}
        }
    }
    super::entrypoints::append(
        &mut types,
        &mut functions,
        &mut exports,
        &mut code,
        imports.len(),
        &super::entrypoints::Calls {
            read: mapping.functions,
            write: mapping.functions + 1,
            settle: settle.ok_or("missing settlement")?,
            post: post.ok_or("missing post-upgrade")?,
            append: append.ok_or("missing reply append")?,
            reply: reply.ok_or("missing reply")?,
        },
        profile,
    );
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&memory)
        .section(&globals)
        .section(&exports)
        .section(&code);
    for section in custom {
        module.section(&section);
    }
    let bytes = module.finish();
    wasm::validate(&bytes, LIMIT).map_err(|_| "combined artifact")?;
    Ok(bytes)
}

pub(super) fn admit_final(final_bytes: &[u8], raw: &[u8], profile: Profile) -> Result<()> {
    if final_bytes != prepare(raw, profile)? {
        return Err("final mismatch");
    }
    Ok(())
}
