//! Fixture-specific admission and bootstrap. This is not a general Wasm linker.

use std::borrow::Cow;
use std::collections::BTreeMap;

use ic_host_artifacts::{artifact::Sha256Digest, wasm};
use wasm_encoder::{
    BlockType, CodeSection, CustomSection, DataCountSection, DataSection, ExportKind,
    ExportSection, Function, FunctionSection, Instruction as I, MemArg, Module, RawSection,
    StartSection, TypeSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{DataKind, ExternalKind, Operator, Parser, Payload, TypeRef, ValType};

mod growth;
use growth::Mode;

/// First retained linear-memory address.
pub const BOUNDARY: i32 = 8 * 1024 * 1024;
/// Scalar fixture's format marker, distinct from M0.
pub const MAGIC: i64 = 0x4f50_5255_5354_3031;
/// Scalar fixture's diagnostic payload.
pub const SENTINEL: i64 = 0x1234_5678_7654_3210;
const MARKER: &str = "icp:private enhanced-orthogonal-persistence";
const LIMIT: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
/// Structural admission failure; no artifact is installed on refusal.
pub enum Refusal {
    /// Invalid or oversized Wasm.
    Malformed,
    /// Unexpected memory count, width or bounds.
    Memory,
    /// Inconsistent linker bounds, signatures or export roster.
    Layout,
    /// Compiler output already owns startup.
    ExistingStart,
    /// Imports differ from the selected profile.
    Import,
    /// Data is outside the transient interval or uses unsupported initialization.
    Data,
    /// Unsupported Wasm feature or section.
    Unsupported,
    /// Forbidden indirect call or table mutation.
    IndirectCall,
    /// Unexpected function table or initialization.
    Table,
    /// Compiler-output growth sites differ from the reviewed kernel contract.
    GrowthCount,
    /// Final bytes differ from the deterministic transform of admitted input.
    FinalMismatch,
}

/// Admitted deployable bytes and their source/bounds report.
pub struct Artifact {
    /// Complete transformed Wasm.
    pub bytes: Vec<u8>,
    /// Input/final hashes and linker bounds.
    pub report: String,
}

fn parsed<T>(result: wasmparser::Result<T>) -> Result<T, Refusal> {
    result.map_err(|_| Refusal::Malformed)
}

fn const_i32(expr: &wasmparser::ConstExpr<'_>) -> Result<i32, Refusal> {
    let mut ops = expr.get_operators_reader();
    match (parsed(ops.read())?, parsed(ops.read())?, ops.eof()) {
        (Operator::I32Const { value }, Operator::End, true) => Ok(value),
        _ => Err(Refusal::Layout),
    }
}

/// Admit the trusted fixture's compiler output, then construct the only allowed
/// bootstrap. Source review remains required: this is not a sandbox for hostile
/// Rust code or a proof that arbitrary stores cannot address retained memory.
pub fn prepare(raw: &[u8]) -> Result<Artifact, Refusal> {
    prepare_profile(raw, false, false)
}

/// Admit and transform the installed graph qualification profile.
pub fn prepare_graph(raw: &[u8]) -> Result<Artifact, Refusal> {
    prepare_profile(raw, true, false)
}

/// Admit and transform the graph benchmark, including argument/counter bindings.
pub fn prepare_benchmark(raw: &[u8]) -> Result<Artifact, Refusal> {
    prepare_profile(raw, true, true)
}

fn prepare_profile(raw: &[u8], managed: bool, benchmark: bool) -> Result<Artifact, Refusal> {
    wasm::validate(raw, LIMIT).map_err(|_| Refusal::Malformed)?;
    let mut sections = Vec::new();
    let mut types = TypeSection::new();
    let mut signatures = Vec::new();
    let mut functions = FunctionSection::new();
    let mut function_types = Vec::new();
    let mut imports = BTreeMap::new();
    let mut globals = Vec::new();
    let mut exports = BTreeMap::new();
    let mut segments = Vec::new();
    let mut bodies = Vec::new();
    let mut memory_seen = false;
    let mut growth_instructions = 0;
    let mut table_size = 0;
    let mut table_initialized = false;
    for payload in Parser::new(0).parse_all(raw) {
        let payload = parsed(payload)?;
        if let Some((id, range)) = payload.as_section() {
            sections.push((id, range));
        }
        match payload {
            Payload::TypeSection(reader) => {
                for ty in reader.clone().into_iter_err_on_gc_types() {
                    signatures.push(parsed(ty)?);
                }
                RoundtripReencoder
                    .parse_type_section(&mut types, reader)
                    .map_err(|_| Refusal::Unsupported)?;
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = parsed(import)?;
                    let TypeRef::Func(ty) = import.ty else {
                        return Err(Refusal::Import);
                    };
                    if import.module != "ic0"
                        || !(matches!(import.name, "msg_reply" | "msg_reply_data_append")
                            || benchmark
                                && matches!(
                                    import.name,
                                    "msg_arg_data_size"
                                        | "msg_arg_data_copy"
                                        | "performance_counter"
                                ))
                    {
                        return Err(Refusal::Import);
                    }
                    let index = imports.len() as u32;
                    if imports.insert(import.name, (index, ty)).is_some() {
                        return Err(Refusal::Import);
                    }
                    function_types.push(ty);
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    let ty = parsed(ty)?;
                    functions.function(ty);
                    function_types.push(ty);
                }
            }
            Payload::MemorySection(reader) => {
                if reader.count() != 1 {
                    return Err(Refusal::Memory);
                }
                for memory in reader {
                    let m = parsed(memory)?;
                    if m.memory64
                        || m.shared
                        || m.initial != 129
                        || m.maximum != Some(256)
                        || m.page_size_log2.is_some()
                    {
                        return Err(Refusal::Memory);
                    }
                }
                memory_seen = true;
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = parsed(global)?;
                    if global.ty.content_type != ValType::I32 {
                        return Err(Refusal::Layout);
                    }
                    globals.push((const_i32(&global.init_expr)?, global.ty.mutable));
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = parsed(export)?;
                    exports.insert(export.name, (export.kind, export.index));
                }
            }
            Payload::StartSection { .. } => return Err(Refusal::ExistingStart),
            Payload::DataCountSection { .. } => return Err(Refusal::Unsupported),
            Payload::DataSection(reader) => {
                for datum in reader {
                    let datum = parsed(datum)?;
                    let DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = datum.kind
                    else {
                        return Err(Refusal::Data);
                    };
                    let offset = const_i32(&offset_expr)?;
                    if offset < 1024 * 1024
                        || (offset as u64 + datum.data.len() as u64) > BOUNDARY as u64
                    {
                        return Err(Refusal::Data);
                    }
                    segments.push((offset, datum.data));
                }
            }
            Payload::CodeSectionEntry(body) => {
                for op in body
                    .get_operators_reader()
                    .map_err(|_| Refusal::Malformed)?
                {
                    let op = parsed(op)?;
                    if matches!(op, Operator::MemoryGrow { mem: 0 }) {
                        growth_instructions += 1;
                        if !managed {
                            return Err(Refusal::Unsupported);
                        }
                    }
                    if matches!(
                        op,
                        Operator::ReturnCallIndirect { .. }
                            | Operator::CallRef { .. }
                            | Operator::ReturnCallRef { .. }
                            | Operator::TableSet { .. }
                            | Operator::TableGrow { .. }
                            | Operator::TableFill { .. }
                            | Operator::TableCopy { .. }
                            | Operator::TableInit { .. }
                    ) {
                        return Err(Refusal::IndirectCall);
                    }
                    if let Operator::CallIndirect { table_index, .. } = op
                        && (!managed || table_index != 0)
                    {
                        return Err(Refusal::IndirectCall);
                    }
                }
                bodies.push(body.as_bytes());
            }
            Payload::TableSection(reader) => {
                // Scalar fixtures have only a null table. The managed profile
                // admits the pinned std panic/formatting table as transient code
                // state; allocator requests trap and table mutation is forbidden.
                if reader.count() > 1 {
                    return Err(Refusal::Unsupported);
                }
                for table in reader {
                    let table = parsed(table)?;
                    let expected_size = if managed { 18 } else { 1 };
                    if table.ty.initial != expected_size
                        || table.ty.maximum != Some(expected_size)
                        || table.ty.table64
                        || table.ty.shared
                        || table.ty.element_type != wasmparser::RefType::FUNCREF
                        || !matches!(table.init, wasmparser::TableInit::RefNull)
                    {
                        return Err(Refusal::Table);
                    }
                    table_size = expected_size;
                }
            }
            Payload::ElementSection(reader) => {
                for element in reader {
                    if !managed || table_initialized || table_size != 18 {
                        return Err(Refusal::Table);
                    }
                    let element = parsed(element)?;
                    let wasmparser::ElementKind::Active {
                        table_index,
                        offset_expr,
                    } = element.kind
                    else {
                        return Err(Refusal::Table);
                    };
                    if table_index.unwrap_or(0) != 0 || const_i32(&offset_expr)? != 1 {
                        return Err(Refusal::Table);
                    }
                    let wasmparser::ElementItems::Functions(items) = element.items else {
                        return Err(Refusal::Table);
                    };
                    if items.count() != 17 {
                        return Err(Refusal::Table);
                    }
                    for index in items {
                        let index = parsed(index)? as usize;
                        if index < imports.len() || index >= function_types.len() {
                            return Err(Refusal::Table);
                        }
                    }
                    table_initialized = true;
                }
            }
            Payload::CustomSection(_)
            | Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::End(_) => {}
            _ => return Err(Refusal::Unsupported),
        }
    }
    if !memory_seen || segments.is_empty() || imports.len() != if benchmark { 5 } else { 2 } {
        return Err(Refusal::Layout);
    }
    // The reviewed graph kernel owns exactly one direct memory.grow. Additional
    // growth sites (for example an accidentally linked general allocator) refuse.
    if growth_instructions != usize::from(managed) {
        return Err(Refusal::GrowthCount);
    }
    if managed && !table_initialized {
        return Err(Refusal::Table);
    }
    for (name, params) in [
        ("msg_reply", &[][..]),
        ("msg_reply_data_append", &[ValType::I32, ValType::I32][..]),
    ] {
        let (_, index) = imports.get(name).ok_or(Refusal::Import)?;
        let ty = signatures.get(*index as usize).ok_or(Refusal::Import)?;
        if ty.params() != params || !ty.results().is_empty() {
            return Err(Refusal::Import);
        }
    }
    if benchmark {
        for (name, params, results) in [
            ("msg_arg_data_size", &[][..], &[ValType::I32][..]),
            (
                "msg_arg_data_copy",
                &[ValType::I32, ValType::I32, ValType::I32][..],
                &[][..],
            ),
            (
                "performance_counter",
                &[ValType::I32][..],
                &[ValType::I64][..],
            ),
        ] {
            let (_, index) = imports.get(name).ok_or(Refusal::Import)?;
            let ty = signatures.get(*index as usize).ok_or(Refusal::Import)?;
            if ty.params() != params || ty.results() != results {
                return Err(Refusal::Import);
            }
        }
    }
    let mut expected_exports = vec![
        "memory",
        "__stack_pointer",
        "__data_end",
        "__heap_base",
        "canister_init",
        "canister_post_upgrade",
        "canister_query transient",
        "canister_update dirty",
    ];
    if managed {
        expected_exports.extend([
            "canister_query graph",
            "canister_query graph_bytes",
            "canister_update graph_change",
            "canister_update graph_grow",
            "canister_update graph_trap",
            "canister_update graph_fill",
        ]);
    }
    if benchmark {
        expected_exports.extend(["canister_update bench_run", "canister_query bench_state"]);
    }
    if exports.len() != expected_exports.len()
        || expected_exports
            .iter()
            .any(|name| !exports.contains_key(name))
        || exports["memory"] != (ExternalKind::Memory, 0)
    {
        return Err(Refusal::Layout);
    }
    let mut bounds = Vec::new();
    for name in ["__stack_pointer", "__data_end", "__heap_base"] {
        let (ExternalKind::Global, index) = exports[name] else {
            return Err(Refusal::Layout);
        };
        let &(value, mutable) = globals.get(index as usize).ok_or(Refusal::Layout)?;
        if !(1024 * 1024..BOUNDARY).contains(&value) || mutable != (name == "__stack_pointer") {
            return Err(Refusal::Layout);
        }
        bounds.push(value);
    }
    if bounds[0] != 1024 * 1024
        || bounds[1] > bounds[2]
        || segments
            .iter()
            .any(|(offset, bytes)| *offset as usize + bytes.len() > bounds[1] as usize)
    {
        return Err(Refusal::Layout);
    }
    for name in &expected_exports[4..] {
        let (ExternalKind::Func, index) = exports[name] else {
            return Err(Refusal::Layout);
        };
        let ty = &signatures[function_types[index as usize] as usize];
        if !ty.params().is_empty() || !ty.results().is_empty() {
            return Err(Refusal::Layout);
        }
    }

    let first = function_types.len() as u32;
    let empty_type = types.len();
    types.ty().function([], []);
    for _ in 0..if managed { 1 } else { 9 } {
        functions.function(empty_type);
    }
    let mut out_exports = ExportSection::new();
    for (&name, &(kind, index)) in &exports {
        let kind = match kind {
            ExternalKind::Func => ExportKind::Func,
            ExternalKind::Global => ExportKind::Global,
            ExternalKind::Memory => ExportKind::Memory,
            _ => return Err(Refusal::Layout),
        };
        out_exports.export(
            name,
            kind,
            if !managed && name == "canister_post_upgrade" {
                first + 3
            } else {
                index
            },
        );
    }
    if !managed {
        out_exports.export("canister_update retain", ExportKind::Func, first + 1);
        out_exports.export("canister_query retained", ExportKind::Func, first + 2);
        for (name, offset) in [
            ("canister_update grow", 4),
            ("canister_update grow_limit", 5),
            ("canister_update grow_then_trap", 6),
            ("canister_query growth", 7),
            ("canister_update corrupt_extent", 8),
        ] {
            out_exports.export(name, ExportKind::Func, first + offset);
        }
    }
    let mut code = CodeSection::new();
    for body in bodies {
        code.raw(body);
    }
    let mut start = Function::new([]);
    for op in [
        I::I32Const(0),
        I::I32Const(0),
        I::I32Const(BOUNDARY),
        I::MemoryFill(0),
    ] {
        start.instruction(&op);
    }
    let mut data = DataSection::new();
    for (i, &(offset, bytes)) in segments.iter().enumerate() {
        data.passive(bytes.iter().copied());
        for op in [
            I::I32Const(offset),
            I::I32Const(0),
            I::I32Const(bytes.len() as i32),
            I::MemoryInit {
                mem: 0,
                data_index: i as u32,
            },
        ] {
            start.instruction(&op);
        }
    }
    start.instruction(&I::End);
    code.function(&start);
    if !managed {
        let mem = MemArg {
            offset: 0,
            align: 3,
            target: wasm_encoder::MemArgTarget::Memory(0),
            ordering: None,
        };
        let mut retain = Function::new([]);
        for op in [
            I::I32Const(BOUNDARY),
            I::I64Const(MAGIC),
            I::I64Store(mem),
            I::I32Const(BOUNDARY + 8),
            I::I64Const(SENTINEL),
            I::I64Store(mem),
            I::I32Const(BOUNDARY + 16),
            I::MemorySize(0),
            I::I64ExtendI32U,
            I::I64Store(mem),
            I::Call(imports["msg_reply"].0),
            I::End,
        ] {
            retain.instruction(&op);
        }
        code.function(&retain);
        let mut retained = Function::new([]);
        for op in [
            I::I32Const(BOUNDARY),
            I::I32Const(16),
            I::Call(imports["msg_reply_data_append"].0),
            I::Call(imports["msg_reply"].0),
            I::End,
        ] {
            retained.instruction(&op);
        }
        code.function(&retained);
        let mut post = Function::new([]);
        for op in [
            I::I32Const(BOUNDARY),
            I::I64Load(mem),
            I::I64Const(MAGIC),
            I::I64Ne,
            I::If(BlockType::Empty),
            I::Unreachable,
            I::End,
            I::I32Const(BOUNDARY + 16),
            I::I64Load(mem),
            I::MemorySize(0),
            I::I64ExtendI32U,
            I::I64Ne,
            I::If(BlockType::Empty),
            I::Unreachable,
            I::End,
            I::Call(exports["canister_post_upgrade"].1),
            I::End,
        ] {
            post.instruction(&op);
        }
        code.function(&post);
        for mode in [Mode::One, Mode::Limit, Mode::Trap] {
            code.function(&growth::grow(
                mode,
                imports["msg_reply_data_append"].0,
                imports["msg_reply"].0,
            ));
        }
        code.function(&growth::status(
            imports["msg_reply_data_append"].0,
            imports["msg_reply"].0,
        ));
        code.function(&growth::corrupt_extent(imports["msg_reply"].0));
    }

    let mut module = Module::new();
    for (id, range) in sections {
        match id {
            0 => {} // Strip compiler metadata; the report binds the original bytes.
            1 => {
                module.section(&types);
            }
            3 => {
                module.section(&functions);
            }
            7 => {
                module.section(&out_exports);
                module.section(&StartSection {
                    function_index: first,
                });
            }
            10 => {
                module.section(&DataCountSection {
                    count: segments.len() as u32,
                });
                module.section(&code);
            }
            11 => {
                module.section(&data);
            }
            _ => {
                module.section(&RawSection {
                    id,
                    data: &raw[range.start as usize..range.end as usize],
                });
            }
        }
    }
    module.section(&CustomSection {
        name: Cow::Borrowed(MARKER),
        data: Cow::Borrowed(if benchmark {
            b"rust-managed-benchmark-v1"
        } else if managed {
            b"rust-managed-graph-v1"
        } else {
            b"rust-startup-probe-v2"
        }),
    });
    let bytes = module.finish();
    wasm::validate(&bytes, LIMIT).map_err(|_| Refusal::Malformed)?;
    let report = format!(
        "input={}\nfinal={}\nstack_top={}\ndata_end={}\nheap_base={}\nretained_base={}\ndata_segments={}\n",
        Sha256Digest::compute(raw),
        Sha256Digest::compute(&bytes),
        bounds[0],
        bounds[1],
        bounds[2],
        BOUNDARY,
        segments.len()
    );
    Ok(Artifact { bytes, report })
}

/// Admission is tied to the reviewed raw module and deterministic transform,
/// including exact startup instructions, lifecycle wrapper and passive segments.
pub fn admit(final_bytes: &[u8], raw: &[u8]) -> Result<(), Refusal> {
    wasm::validate(final_bytes, LIMIT).map_err(|_| Refusal::Malformed)?;
    if prepare(raw)?.bytes != final_bytes {
        return Err(Refusal::FinalMismatch);
    }
    Ok(())
}

/// Bind final graph bytes to their reviewed raw source artifact.
pub fn admit_graph(final_bytes: &[u8], raw: &[u8]) -> Result<(), Refusal> {
    wasm::validate(final_bytes, LIMIT).map_err(|_| Refusal::Malformed)?;
    if prepare_graph(raw)?.bytes != final_bytes {
        return Err(Refusal::FinalMismatch);
    }
    Ok(())
}
