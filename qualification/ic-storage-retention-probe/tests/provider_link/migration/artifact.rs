//! Compose two reviewed compiler domains. Only scalar callbacks cross domains.

use ic_host_artifacts::wasm;
use std::collections::{BTreeMap, BTreeSet};
use wasm_encoder::{
    CodeSection, DataCountSection, DataSection, ElementSection, ExportKind, ExportSection,
    Function, FunctionSection, GlobalSection, ImportSection, Instruction as I, MemorySection,
    Module, RawSection, TableSection, TypeSection, reencode::Reencode,
};
use wasmparser::{DataKind, ExternalKind, Operator as Op, Parser, Payload, TypeRef};

struct Inventory {
    imports: Vec<(String, String)>,
    functions: u32,
    types: u32,
    globals: u32,
    exports: BTreeMap<String, (ExternalKind, u32)>,
}

fn inventory(bytes: &[u8], bridge: bool) -> Inventory {
    wasm::validate(bytes, super::LIMIT).unwrap();
    let mut result = Inventory {
        imports: vec![],
        functions: 0,
        types: 0,
        globals: 0,
        exports: BTreeMap::new(),
    };
    let mut values = Vec::new();
    let mut ranges = Vec::new();
    let mut memories = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::TypeSection(r) => result.types = r.count(),
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                    assert!(
                        import.module == "ic0" || (bridge && import.module == "native_checkpoint")
                    );
                    result
                        .imports
                        .push((import.module.into(), import.name.into()));
                }
            }
            Payload::FunctionSection(r) => result.functions = r.count(),
            Payload::MemorySection(r) => {
                for memory in r {
                    let memory = memory.unwrap();
                    assert!(!memory.memory64 && !memory.shared && memory.page_size_log2.is_none());
                    assert_eq!((memory.initial, memory.maximum), (32, Some(256)));
                    memories += 1;
                }
            }
            Payload::GlobalSection(r) => {
                result.globals = r.count();
                for global in r {
                    let global = global.unwrap();
                    values.push(
                        match global.init_expr.get_operators_reader().read().unwrap() {
                            Op::I32Const { value } => Some(value as u32),
                            _ => None,
                        },
                    );
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    assert!(
                        result
                            .exports
                            .insert(export.name.into(), (export.kind, export.index))
                            .is_none()
                    );
                }
            }
            Payload::DataSection(r) => {
                for data in r {
                    let data = data.unwrap();
                    let DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = data.kind
                    else {
                        panic!("active memory-zero data only")
                    };
                    let mut reader = offset_expr.get_operators_reader();
                    let Op::I32Const { value } = reader.read().unwrap() else {
                        panic!("constant data offset")
                    };
                    assert!(matches!(reader.read().unwrap(), Op::End) && reader.eof());
                    ranges.push((value as u64, value as u64 + data.data.len() as u64));
                }
            }
            Payload::StartSection { .. } => panic!("no implicit startup"),
            _ => {}
        }
    }
    assert_eq!(memories, 1);
    if bridge {
        let global = |name: &str| {
            let (kind, index) = result.exports[name];
            assert_eq!(kind, ExternalKind::Global);
            values[index as usize].unwrap()
        };
        assert_eq!(global("__stack_pointer"), 65536);
        let end = global("__data_end");
        let heap = global("__heap_base");
        assert!(196608 <= end && end <= heap && heap < 2097152);
        assert!(!ranges.is_empty());
        for (start, finish) in ranges {
            assert!(196608 <= start && start <= finish && finish <= u64::from(end));
        }
        let mut callbacks: Vec<_> = result
            .imports
            .iter()
            .filter(|(m, _)| m == "native_checkpoint")
            .map(|(_, n)| n.as_str())
            .collect();
        callbacks.sort_unstable();
        let mut expected = vec![
            "begin", "counter", "finish", "freeze", "fresh", "put", "value",
        ];
        if callbacks.contains(&"domain") {
            expected.insert(2, "domain");
            expected.insert(7, "thaw");
            expected.extend([
                "stage_begin",
                "stage_count",
                "stage_counter",
                "stage_damage",
                "stage_value",
                "stage_put",
                "stage_finish",
                "stage_ready",
            ]);
            expected.sort_unstable();
        }
        if callbacks.contains(&"record_schema") {
            assert!(!callbacks.contains(&"domain"));
            expected.extend([
                "record_schema",
                "record_ceiling",
                "record_restore",
                "record_source",
                "record_publish",
                "record_read",
                "record_write",
            ]);
            expected.sort_unstable();
        }
        assert_eq!(callbacks, expected);
        assert_eq!(
            result.exports.contains_key("checkpoint_convert"),
            callbacks.contains(&"record_schema")
        );
        assert_eq!(
            result.exports.contains_key("checkpoint_narrow"),
            callbacks.contains(&"record_schema")
        );
        assert_eq!(
            result.exports.contains_key("checkpoint_record_add"),
            callbacks.contains(&"record_schema")
        );
        assert_eq!(
            result.exports.contains_key("checkpoint_cancel"),
            callbacks.contains(&"domain")
        );
        assert_eq!(
            result.exports.contains_key("checkpoint_resume"),
            callbacks.contains(&"domain")
        );
        for name in [
            "checkpoint_stage",
            "checkpoint_stage_resume",
            "checkpoint_stage_restart",
            "checkpoint_batch",
            "checkpoint_stage_damage",
        ] {
            assert_eq!(
                result.exports.contains_key(name),
                callbacks.contains(&"domain")
            );
        }
        for name in result.exports.keys() {
            assert!(matches!(
                name.as_str(),
                "memory"
                    | "__stack_pointer"
                    | "__data_end"
                    | "__heap_base"
                    | "checkpoint_create"
                    | "checkpoint_export"
                    | "checkpoint_restore"
                    | "checkpoint_bytes"
                    | "checkpoint_damage"
                    | "checkpoint_cancel"
                    | "checkpoint_resume"
                    | "checkpoint_stage"
                    | "checkpoint_stage_resume"
                    | "checkpoint_stage_restart"
                    | "checkpoint_batch"
                    | "checkpoint_stage_damage"
                    | "checkpoint_convert"
                    | "checkpoint_narrow"
                    | "checkpoint_record_add"
            ));
        }
    } else {
        assert!(ranges.is_empty());
        let bodies: Vec<_> = Parser::new(0)
            .parse_all(bytes)
            .filter_map(|payload| {
                if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                    Some(body)
                } else {
                    None
                }
            })
            .collect();
        let imported = result.imports.len() as u32;
        let first_rust =
            imported + result.functions - super::super::vector_access::NATIVE_FUNCTIONS;
        let mut pending: Vec<_> = result
            .exports
            .iter()
            .filter(|(name, _)| {
                name.starts_with("ncp_")
                    || matches!(
                        name.as_str(),
                        "__bridge_frozen" | "__bridge_importing" | "__bridge_record_source"
                    )
            })
            .map(|(_, (_, index))| *index)
            .collect();
        assert_eq!(
            pending.len(),
            if result.exports.contains_key("ncp_domain") {
                19
            } else if result.exports.contains_key("ncp_record_schema") {
                15
            } else {
                7
            }
        );
        let mut seen = BTreeSet::new();
        while let Some(index) = pending.pop() {
            assert!(
                index >= imported && index < first_rust,
                "scalar callback re-enters Rust or host"
            );
            if !seen.insert(index) {
                continue;
            }
            for op in bodies[(index - imported) as usize]
                .get_operators_reader()
                .unwrap()
            {
                match op.unwrap() {
                    Op::Call { function_index } => pending.push(function_index),
                    Op::CallIndirect { .. }
                    | Op::ReturnCall { .. }
                    | Op::ReturnCallIndirect { .. }
                    | Op::CallRef { .. }
                    | Op::ReturnCallRef { .. }
                    | Op::MemoryGrow { .. } => panic!("unreviewed callback control"),
                    _ => {}
                }
            }
        }
    }
    result
}

struct Mapping {
    functions: Vec<u32>,
    types: u32,
    globals: u32,
}
impl Reencode for Mapping {
    type Error = std::convert::Infallible;
    fn function_index(
        &mut self,
        index: u32,
    ) -> Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.functions[index as usize])
    }
    fn type_index(
        &mut self,
        index: u32,
    ) -> Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.types + index)
    }
    fn global_index(
        &mut self,
        index: u32,
    ) -> Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.globals + index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Post {
    Refuse,
    Recover,
    Reopen,
    Frozen,
    Stage,
    Restart,
    Importing,
    Convert,
    Narrow,
}

pub(super) fn combine(native: &[u8], bridge: &[u8], post: Post) -> Vec<u8> {
    let p = inventory(bridge, true);
    let n = inventory(native, false);
    assert_eq!(
        p.exports.contains_key("checkpoint_convert"),
        n.exports.contains_key("ncp_record_schema")
    );
    assert_eq!(
        n.exports.contains_key("ncp_thaw"),
        n.exports.contains_key("ncp_domain")
    );
    assert_eq!(
        n.exports.contains_key("__bridge_frozen"),
        n.exports.contains_key("ncp_domain")
    );
    assert_eq!(
        n.exports.contains_key("__bridge_importing"),
        n.exports.contains_key("ncp_domain")
    );
    assert_eq!(
        p.imports
            .iter()
            .any(|(m, n)| m == "native_checkpoint" && n == "domain"),
        n.exports.contains_key("ncp_domain")
    );
    let reset = matches!(
        post,
        Post::Reopen | Post::Frozen | Post::Importing | Post::Convert | Post::Narrow
    )
    .then(|| super::transient::Plan::read(bridge));
    let ic: Vec<_> = p
        .imports
        .iter()
        .filter(|(m, _)| m == "ic0")
        .cloned()
        .collect();
    let pbase = ic.len() as u32;
    let nbase = pbase + p.functions;
    let nindex = |index: u32| {
        if index < n.imports.len() as u32 {
            ic.iter()
                .position(|item| *item == n.imports[index as usize])
                .unwrap() as u32
        } else {
            nbase + index - n.imports.len() as u32
        }
    };
    let mut pm = Mapping {
        functions: p
            .imports
            .iter()
            .map(|item| {
                if item.0 == "ic0" {
                    ic.iter().position(|i| i == item).unwrap() as u32
                } else {
                    nindex(n.exports[&format!("ncp_{}", item.1)].1)
                }
            })
            .chain(pbase..pbase + p.functions)
            .collect(),
        types: 0,
        globals: 0,
    };
    let mut nm = Mapping {
        functions: (0..n.imports.len() as u32 + n.functions)
            .map(nindex)
            .collect(),
        types: p.types,
        globals: p.globals,
    };
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut memory = MemorySection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut table = TableSection::new();
    let mut elements = ElementSection::new();
    let mut data = DataSection::new();
    let mut data_count = None;
    let mut custom = Vec::new();
    for (bytes, map, primary) in [(bridge, &mut pm, true), (native, &mut nm, false)] {
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::TypeSection(r) => map.parse_type_section(&mut types, r).unwrap(),
                Payload::ImportSection(r) => {
                    if primary {
                        for import in r.into_imports() {
                            let import = import.unwrap();
                            if import.module == "ic0" {
                                map.parse_import(&mut imports, import).unwrap();
                            }
                        }
                    }
                }
                Payload::FunctionSection(r) => {
                    map.parse_function_section(&mut functions, r).unwrap()
                }
                Payload::MemorySection(r) => {
                    if primary {
                        map.parse_memory_section(&mut memory, r).unwrap();
                    }
                }
                Payload::GlobalSection(r) => map.parse_global_section(&mut globals, r).unwrap(),
                Payload::ExportSection(r) => {
                    for export in r {
                        let export = export.unwrap();
                        if (primary && export.name == "memory")
                            || (!primary && export.name.starts_with("canister_"))
                        {
                            map.parse_export(&mut exports, export).unwrap();
                        }
                    }
                }
                Payload::TableSection(r) => {
                    assert!(primary);
                    map.parse_table_section(&mut table, r).unwrap();
                }
                Payload::ElementSection(r) => {
                    assert!(primary);
                    map.parse_element_section(&mut elements, r).unwrap();
                }
                Payload::DataSection(r) => {
                    assert!(primary);
                    map.parse_data_section(&mut data, r).unwrap();
                }
                Payload::DataCountSection { count, .. } => {
                    assert!(primary);
                    data_count = Some(count);
                }
                Payload::CodeSectionEntry(body) => {
                    map.parse_function_body(&mut code, body).unwrap()
                }
                Payload::CustomSection(section) => {
                    if !primary && section.name() == "icp:private enhanced-orthogonal-persistence" {
                        custom.push(section.range());
                    }
                }
                Payload::Version { .. } | Payload::CodeSectionStart { .. } | Payload::End(_) => {}
                _ => panic!("unsupported bridge section"),
            }
        }
    }
    let pcall = |name: &str| pm.functions[p.exports[name].1 as usize];
    let ncall = |name: &str| nm.functions[n.exports[name].1 as usize];
    let unit = types.len();
    types.ty().function([], []);
    let reset_index = imports.len() + functions.len();
    if let Some(reset) = reset {
        functions.function(unit);
        code.function(&reset.append(&mut data));
        data_count = Some(data.len());
    }
    let mut entrypoints = vec![
        (
            "canister_init",
            vec![ncall("__bridge_init"), pcall("checkpoint_create")],
        ),
        (
            "canister_post_upgrade",
            match post {
                Post::Recover => vec![
                    ncall("ncp_fresh"),
                    pcall("checkpoint_restore"),
                    ncall("__bridge_post"),
                ],
                // Native admission runs first without entering checkpoint Rust.
                // Its transient native frame has ended before clearing stack.
                Post::Reopen => vec![ncall("__bridge_post"), reset_index],
                Post::Convert => vec![
                    ncall("__bridge_record_source"),
                    reset_index,
                    pcall("checkpoint_convert"),
                    ncall("__bridge_post"),
                ],
                Post::Narrow => vec![
                    ncall("__bridge_record_source"),
                    reset_index,
                    pcall("checkpoint_narrow"),
                    ncall("__bridge_post"),
                ],
                Post::Frozen => vec![
                    ncall("__bridge_frozen"),
                    reset_index,
                    pcall("checkpoint_resume"),
                ],
                Post::Stage => vec![ncall("ncp_fresh"), pcall("checkpoint_stage")],
                Post::Restart => vec![ncall("ncp_fresh"), pcall("checkpoint_stage_restart")],
                Post::Importing => vec![
                    ncall("__bridge_importing"),
                    reset_index,
                    pcall("checkpoint_stage_resume"),
                ],
                Post::Refuse => vec![],
            },
        ),
        (
            "canister_update export_checkpoint",
            vec![pcall("checkpoint_export")],
        ),
        (
            "canister_query checkpoint_bytes",
            vec![pcall("checkpoint_bytes")],
        ),
        (
            "canister_update damage_checkpoint",
            vec![pcall("checkpoint_damage")],
        ),
    ];
    if p.exports.contains_key("checkpoint_record_add") {
        entrypoints.push((
            "canister_update record_add",
            vec![pcall("checkpoint_record_add")],
        ));
    }
    if p.exports.contains_key("checkpoint_cancel") {
        entrypoints.push((
            "canister_update cancel_checkpoint",
            vec![pcall("checkpoint_cancel")],
        ));
        entrypoints.push((
            "canister_update import_batch",
            vec![pcall("checkpoint_batch")],
        ));
        entrypoints.push((
            "canister_update damage_import",
            vec![pcall("checkpoint_stage_damage")],
        ));
    }
    for (name, calls) in entrypoints {
        exports.export(name, ExportKind::Func, imports.len() + functions.len());
        functions.function(unit);
        let mut body = Function::new([]);
        if name == "canister_post_upgrade" && post == Post::Refuse {
            body.instruction(&I::Unreachable);
        }
        for call in calls {
            body.instruction(&I::Call(call));
        }
        body.instruction(&I::End);
        code.function(&body);
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&table)
        .section(&memory)
        .section(&globals)
        .section(&exports)
        .section(&elements);
    if let Some(count) = data_count {
        module.section(&DataCountSection { count });
    }
    module.section(&code).section(&data);
    for range in custom {
        module.section(&RawSection {
            id: 0,
            data: &native[range.start as usize..range.end as usize],
        });
    }
    module.finish()
}
