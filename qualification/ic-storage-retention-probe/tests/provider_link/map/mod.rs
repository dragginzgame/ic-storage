//! Closed map artifact qualification, separate from scalar/vector admission.

use ic_storage_retention_probe::artifact::sha256_hex;
use std::{collections::BTreeMap, fmt::Write, fs, path::PathBuf};
use wasmparser::{Parser, Payload};

mod installed_tests;
mod link;
mod recovery;

fn inputs() -> (PathBuf, Vec<u8>) {
    let directory =
        PathBuf::from(std::env::var_os("NATIVE_MAP_INPUTS").expect("map collector inputs"));
    assert!(directory.is_absolute());
    let raw = ic_host_artifacts::read(
        fs::File::open(directory.join("provider_map.wasm")).unwrap(),
        ic_host_artifacts::Length::AtMost(super::artifact::LIMIT as u64),
    )
    .unwrap()
    .0;
    (directory, raw)
}

#[test]
#[ignore = "map collector supplies fresh compiler artifacts"]
fn compiled_map_contract() {
    let (directory, raw) = inputs();
    let mut listing = String::new();
    let mut inventory = BTreeMap::<String, usize>::new();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&raw) {
        match payload.unwrap() {
            Payload::CodeSectionEntry(body) => {
                writeln!(listing, "function {}", bodies.len() + 1).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    writeln!(listing, "{op:?}").unwrap();
                    *inventory
                        .entry(
                            format!("{op:?}")
                                .split([' ', '{'])
                                .next()
                                .unwrap()
                                .to_owned(),
                        )
                        .or_default() += 1;
                }
                bodies.push(body);
            }
            Payload::TypeSection(r) => {
                for ty in r.into_iter_err_on_gc_types() {
                    println!("type {:?}", ty.unwrap());
                }
            }
            Payload::FunctionSection(r) => println!(
                "functions {:?}",
                r.into_iter().collect::<Result<Vec<_>, _>>().unwrap()
            ),
            Payload::ExportSection(r) => {
                for export in r {
                    println!("export {:?}", export.unwrap());
                }
            }
            Payload::GlobalSection(r) => {
                for global in r {
                    println!("global {:?}", global.unwrap());
                }
            }
            Payload::DataSection(r) => println!("data sections {}", r.count()),
            _ => {}
        }
    }
    fs::write(directory.join("operators.txt"), listing).unwrap();
    println!("operators={inventory:?}");
    println!(
        "pool stack={:?}",
        super::vector_stack::inspect_map(&bodies, 6, 0, 32768)
    );
    assert!(super::vector_access::inspect(&raw).is_err());
    assert!(super::vector_access::inspect_record(&raw).is_err());
    assert!(super::vector_access::inspect_record_v2(&raw).is_err());
    let profile = super::vector_access::inspect_map(&raw).unwrap();
    assert_eq!((profile.functions, profile.entry), (8, 6));
    let guarded = super::vector_access::prepare_map(&raw).unwrap();
    assert_eq!(
        super::vector_access::tests::verify_map_coverage(&raw, &guarded),
        [167, 13, 34, 82, 11, 54, 17, 0]
    );
    let linked = link::prepare(&raw, link::Profile::A).unwrap();
    link::admit_final(&linked, &raw, link::Profile::A).unwrap();
    assert!(link::admit_final(&linked, &raw, link::Profile::B).is_err());
    assert!(link::admit_final(&linked[..linked.len() - 1], &raw, link::Profile::A).is_err());
    for payload in Parser::new(0).parse_all(&linked) {
        match payload.unwrap() {
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.unwrap();
                    assert!(
                        !export.name.starts_with("__link_")
                            && !export.name.starts_with("provider_")
                    );
                }
            }
            Payload::ImportSection(reader) => {
                let imports = reader
                    .into_imports()
                    .map(|import| {
                        let import = import.unwrap();
                        (import.module, import.name)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    imports,
                    [("ic0", "msg_reply_data_append"), ("ic0", "msg_reply")]
                );
            }
            _ => {}
        }
    }
    fs::write(directory.join("guarded.wasm"), guarded).unwrap();
    fs::write(directory.join("linked.wasm"), &linked).unwrap();
    fs::write(
        directory.join("ADMITTED"),
        format!("raw={} linked={}\n", sha256_hex(&raw), sha256_hex(&linked)),
    )
    .unwrap();
}
