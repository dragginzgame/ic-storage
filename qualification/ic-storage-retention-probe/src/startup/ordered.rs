//! Closed ordered-owner profile under the shared transient startup boundary.
use super::{Artifact, reset};
use crate::artifact::{sha256_hex, validate_wasm};
use std::{borrow::Cow, collections::BTreeSet};
use wasm_encoder::{
    CodeSection, CustomSection, DataCountSection, FunctionSection, Module, RawSection,
    StartSection, TypeSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{DataKind, ExternalKind, Operator, Parser, Payload, TypeRef, ValType};

fn admit_raw(raw: &[u8]) -> Result<(), &'static str> {
    validate_wasm(raw, 1024 * 1024).map_err(|_| "invalid Wasm")?;
    let mut memories = 0;
    let mut exports = BTreeSet::new();
    let mut globals = Vec::new();
    let mut data_end = 0;
    let mut layout = std::collections::BTreeMap::new();
    let mut memory_export = false;
    for payload in Parser::new(0).parse_all(raw) {
        match payload.map_err(|_| "section")? {
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|_| "memory")?;
                    if memory.memory64
                        || memory.shared
                        || memory.initial != 128
                        || memory.maximum != Some(128)
                        || memory.page_size_log2.is_some()
                    {
                        return Err("memory geometry");
                    }
                    memories += 1;
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(|_| "import")?;
                    if import.module != "ic0"
                        || !matches!(import.ty, TypeRef::Func(_))
                        || !matches!(
                            import.name,
                            "msg_arg_data_size"
                                | "msg_arg_data_copy"
                                | "msg_reply_data_append"
                                | "msg_reply"
                                | "trap"
                                | "performance_counter"
                                | "stable64_size"
                                | "stable64_grow"
                                | "stable64_read"
                                | "stable64_write"
                        )
                    {
                        return Err("unapproved import");
                    }
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.map_err(|_| "global")?;
                    let mut ops = global.init_expr.get_operators_reader();
                    let Operator::I32Const { value } = ops.read().map_err(|_| "global value")?
                    else {
                        return Err("nonconstant global");
                    };
                    if global.ty.content_type != ValType::I32
                        || !matches!(ops.read().map_err(|_| "global end")?, Operator::End)
                        || !ops.eof()
                    {
                        return Err("global expression");
                    }
                    globals.push((value as u32, global.ty.mutable));
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|_| "export")?;
                    if export.name.starts_with("canister_") {
                        if export.kind != ExternalKind::Func {
                            return Err("entry type");
                        }
                        exports.insert(export.name.to_owned());
                    } else if matches!(
                        export.name,
                        "__data_end" | "__heap_base" | "__stack_pointer"
                    ) {
                        if export.kind != ExternalKind::Global {
                            return Err("layout export");
                        }
                        let (value, mutable) =
                            *globals.get(export.index as usize).ok_or("layout index")?;
                        if !(1_048_576..1_572_864).contains(&value)
                            || mutable != (export.name == "__stack_pointer")
                            || (export.name == "__stack_pointer" && value != 1_048_576)
                        {
                            return Err("transient compiler geometry");
                        }
                        layout.insert(export.name.to_owned(), value as usize);
                    } else if export.name == "memory" {
                        if export.kind != ExternalKind::Memory || export.index != 0 {
                            return Err("memory export");
                        }
                        memory_export = true;
                    } else {
                        return Err("unexpected export");
                    }
                }
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.map_err(|_| "data")?;
                    let DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = data.kind
                    else {
                        return Err("data initialization");
                    };
                    let mut ops = offset_expr.get_operators_reader();
                    let Operator::I32Const { value } = ops.read().map_err(|_| "data offset")?
                    else {
                        return Err("data expression");
                    };
                    if !matches!(ops.read().map_err(|_| "data end")?, Operator::End) || !ops.eof() {
                        return Err("data expression");
                    }
                    let end = (value as u32 as usize)
                        .checked_add(data.data.len())
                        .ok_or("data overflow")?;
                    if value < 1_048_576 || end > 1_572_864 {
                        return Err("retained initialization overlap");
                    }
                    data_end = data_end.max(end);
                }
            }
            Payload::StartSection { .. } => return Err("unexpected start"),
            Payload::CustomSection(section)
                if section.name() == "icp:private enhanced-orthogonal-persistence" =>
            {
                return Err("raw marker must be added exactly once");
            }
            Payload::CodeSectionEntry(body) => {
                for op in body.get_operators_reader().map_err(|_| "operators")? {
                    match op.map_err(|_| "operator")? {
                        Operator::MemoryGrow { .. } => return Err("unexpected memory growth"),
                        Operator::MemoryInit { .. } | Operator::DataDrop { .. } => {
                            return Err("compiler-owned data lifecycle");
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let expected = [
        "canister_init",
        "canister_pre_upgrade",
        "canister_post_upgrade",
        "canister_update put",
        "canister_update lookup",
        "canister_update delete",
        "canister_update checkpoint",
        "canister_update damage",
        "canister_query records",
        "canister_query observe",
        "canister_query costs",
        "canister_query page",
        "canister_query query_put",
        "canister_query session_status",
        "canister_update session_close",
        "canister_update session_open",
    ]
    .map(str::to_owned)
    .into_iter()
    .collect();
    if memories != 1 || !memory_export || layout.len() != 3 || data_end == 0 || exports != expected
    {
        return Err("closed export/memory surface");
    }
    if data_end > layout["__data_end"] || layout["__data_end"] > layout["__heap_base"] {
        return Err("linker bounds");
    }
    Ok(())
}

/// Admit the source-reviewed ordered fixture and discard all generation-local
/// Rust headers before replaying compiler initialization. This geometry screen
/// does not verify arbitrary Wasm stores or grant general application custody.
pub fn prepare_ordered(raw: &[u8]) -> Result<Artifact, &'static str> {
    admit_raw(raw)?;
    let mut sections = Vec::new();
    let mut types = TypeSection::new();
    let mut functions = FunctionSection::new();
    let mut code = CodeSection::new();
    let mut segments = Vec::new();
    let mut first = 0;
    for payload in Parser::new(0).parse_all(raw) {
        let payload = payload.map_err(|_| "section")?;
        if let Some((id, range)) = payload.as_section() {
            sections.push((id, range));
        }
        match payload {
            Payload::TypeSection(reader) => RoundtripReencoder
                .parse_type_section(&mut types, reader)
                .map_err(|_| "types")?,
            Payload::ImportSection(reader) => first += reader.into_imports().count() as u32,
            Payload::FunctionSection(reader) => {
                first += reader.count();
                for ty in reader {
                    functions.function(ty.map_err(|_| "function")?);
                }
            }
            Payload::CodeSectionEntry(body) => {
                code.raw(body.as_bytes());
            }
            Payload::DataSection(reader) => {
                for segment in reader {
                    let segment = segment.map_err(|_| "data")?;
                    let DataKind::Active { offset_expr, .. } = segment.kind else {
                        return Err("data");
                    };
                    let Operator::I32Const { value } = offset_expr
                        .get_operators_reader()
                        .read()
                        .map_err(|_| "offset")?
                    else {
                        return Err("offset");
                    };
                    segments.push((value, segment.data));
                }
            }
            _ => {}
        }
    }
    functions.function(types.len());
    types.ty().function([], []);
    // Compiler headers lie below HEADER. Both native windows and the scalar
    // descriptors at HEADER remain untouched. Scratch has no inter-entry owners.
    let (start, data) = reset::build(1_572_864, &segments);
    code.function(&start);
    let mut module = Module::new();
    for (id, range) in sections {
        match id {
            0 | 12 => {}
            1 => {
                module.section(&types);
            }
            3 => {
                module.section(&functions);
            }
            7 => {
                module.section(&RawSection {
                    id,
                    data: &raw[range.start as usize..range.end as usize],
                });
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
        name: Cow::Borrowed("icp:private enhanced-orthogonal-persistence"),
        data: Cow::Borrowed(b"ordered-owner-session-v1"),
    });
    let bytes = module.finish();
    validate_wasm(&bytes, 1024 * 1024).map_err(|_| "transformed Wasm")?;
    let report = format!(
        "input={}\nfinal={}\ntransient_end=1572864\ndata_segments={}\n",
        sha256_hex(raw),
        sha256_hex(&bytes),
        segments.len()
    );
    Ok(Artifact { bytes, report })
}

/// Bind the deployable artifact to the exact admitted input and deterministic startup.
pub fn admit_ordered(final_bytes: &[u8], raw: &[u8]) -> Result<(), &'static str> {
    if prepare_ordered(raw)?.bytes != final_bytes {
        return Err("final mismatch");
    }
    Ok(())
}
