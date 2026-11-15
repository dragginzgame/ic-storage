//! Inventory vector compiler output, without linking or admitting native access.

use std::{collections::BTreeMap, fmt::Write, fs};

use ic_host_artifacts::{
    artifact::{Sha256Digest, read_reader},
    wasm,
};
use wasmparser::{DataKind, ExternalKind, Operator, Parser, Payload, TypeRef};

#[test]
#[ignore = "make native-provider-probe supplies fresh vector compiler output"]
fn compiled_vector_has_bounded_transient_storage_but_is_not_scalar_admitted() {
    let (directory, _) = super::input();
    let raw = read_reader(
        fs::File::open(directory.join("vector/provider_vector.wasm")).unwrap(),
        super::artifact::LIMIT,
    )
    .unwrap();
    wasm::validate(&raw, super::artifact::LIMIT).unwrap();
    assert!(super::artifact::admit_raw(&raw).is_err());
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    let mut globals = Vec::new();
    let mut bodies = 0;
    let mut function_bodies = Vec::new();
    let mut data_bytes = 0;
    let mut memories = 0;
    let mut stack_updates = 0;
    let mut copies = 0;
    let mut listing = String::new();
    for payload in Parser::new(0).parse_all(&raw) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                    imports.push((import.module, import.name));
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    exports.insert(export.name, (export.kind, export.index));
                }
            }
            Payload::MemorySection(r) => {
                for memory in r {
                    let memory = memory.unwrap();
                    assert_eq!((memory.initial, memory.maximum), (3, Some(3)));
                    assert!(!memory.memory64 && !memory.shared);
                    memories += 1;
                }
            }
            Payload::GlobalSection(r) => {
                for global in r {
                    let global = global.unwrap();
                    let mut ops = global.init_expr.get_operators_reader();
                    let Operator::I32Const { value } = ops.read().unwrap() else {
                        panic!("unexpected boundary initializer");
                    };
                    assert!(matches!(ops.read().unwrap(), Operator::End));
                    globals.push(value);
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
                        panic!("unexpected data mode");
                    };
                    let Operator::I32Const { value } =
                        offset_expr.get_operators_reader().read().unwrap()
                    else {
                        panic!("unexpected data offset");
                    };
                    assert!(value >= 32768);
                    assert!((value as usize).checked_add(data.data.len()).unwrap() <= 65536);
                    data_bytes += data.data.len();
                }
            }
            Payload::CodeSectionEntry(body) => {
                function_bodies.push(body.clone());
                bodies += 1;
                writeln!(
                    listing,
                    "function {} locals {:?}",
                    imports.len() + bodies - 1,
                    body.get_locals_reader()
                        .unwrap()
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap()
                )
                .unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    writeln!(listing, "{op:?}").unwrap();
                    match op {
                        Operator::MemoryGrow { .. } => panic!("unexpected growth authority"),
                        Operator::GlobalSet { .. } => stack_updates += 1,
                        Operator::MemoryCopy { .. } => copies += 1,
                        _ => {}
                    }
                }
            }
            Payload::StartSection { .. } => panic!("unexpected startup function"),
            _ => {}
        }
    }
    fs::write(directory.join("vector/operators.txt"), listing).unwrap();
    assert_eq!(imports, [("native_vector_diagnostic", "claim_region")]);
    assert_eq!(memories, 1);
    assert_eq!(exports["provider_vector_step"].0, ExternalKind::Func);
    for name in ["__stack_pointer", "__data_end", "__heap_base"] {
        let (kind, index) = exports[name];
        assert_eq!(kind, ExternalKind::Global);
        let value = globals[index as usize];
        assert!((32768..=65536).contains(&value));
        println!("vector {name}={value}");
    }
    assert!(
        stack_updates > 0,
        "vector profile must cover transient stack use"
    );
    let (_, stack_global) = exports["__stack_pointer"];
    let report = super::vector_stack::inspect(
        &function_bodies,
        exports["provider_vector_step"].1,
        stack_global,
        u32::try_from(globals[stack_global as usize]).unwrap(),
    )
    .unwrap();
    println!("vector stack/call diagnostic: {report:?}; payload access remains unqualified");
    let guarded = super::vector_access::prepare(&raw).unwrap();
    let coverage = super::vector_access::tests::verify_coverage(&raw, &guarded);
    fs::write(directory.join("vector/guarded.wasm"), &guarded).unwrap();
    println!(
        "vector access rewrite: sites={coverage:?} bytes={} sha256={}; unresolved helpers, NOT executed",
        guarded.len(),
        Sha256Digest::compute(&guarded)
    );
    println!(
        "vector diagnostic sha256={} bytes={} functions={bodies} data_bytes={data_bytes} global_sets={stack_updates} memory_copies={copies}; NOT admitted or executed",
        Sha256Digest::compute(&raw),
        raw.len(),
    );
}
