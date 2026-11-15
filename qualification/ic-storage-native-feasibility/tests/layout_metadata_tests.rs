//! Compare compiler-emitted diagnostics; never admit a retained representation.

use std::{collections::BTreeMap, env, fs};

mod metadata;

#[test]
#[ignore = "make native-layout-probe supplies fresh host/wasm LLVM IR"]
fn compiled_layouts() {
    let directory = env::var("NATIVE_LAYOUT_INPUTS").expect("explicit diagnostic input directory");
    for target in ["host", "wasm"] {
        let mut reports = Vec::new();
        for variant in ["base", "swapped"] {
            let source = fs::read_to_string(format!("{directory}/{target}-{variant}.ll")).unwrap();
            let report = metadata::inspect(&source).expect("complete supported diagnostic shape");
            println!("{target}-{variant} (sizes/alignments/parent-relative offsets in bits):");
            for field in &report {
                println!("{field:?}");
            }
            assert!(report.iter().any(|f| f.path == "record.values.buf.inner.ptr.pointer" && f.kind == "pointer"));
            assert!(
                report
                    .iter()
                    .any(|f| f.path == "record.values.buf.inner.cap.__0" && f.kind == "scalar")
            );
            assert!(
                report
                    .iter()
                    .any(|f| f.path == "record.values.len" && f.kind == "scalar")
            );
            assert!(
                report
                    .iter()
                    .any(|f| f.path == "record.values.buf.inner.alloc" && f.size_bits == 0)
            );
            if target == "host" {
                let observed =
                    fs::read_to_string(format!("{directory}/host-{variant}-observation.txt"))
                        .unwrap();
                let mut numbers = BTreeMap::new();
                for item in observed.split_whitespace() {
                    let (key, value) = item.split_once('=').expect("key=value observation");
                    assert!(numbers.insert(key, value.parse::<u64>().unwrap()).is_none());
                }
                assert_eq!(numbers.len(), 8);
                let root = &report[0];
                let counter = report.iter().find(|f| f.path == "record.counter").unwrap();
                let values = report.iter().find(|f| f.path == "record.values").unwrap();
                assert_eq!(root.size_bits, numbers["record_size"] * 8);
                assert_eq!(root.align_bits, Some(numbers["record_align"] * 8));
                assert_eq!(counter.offset_bits, numbers["counter_offset"] * 8);
                assert_eq!(values.offset_bits, numbers["values_offset"] * 8);
                assert_eq!(values.size_bits, numbers["vector_size"] * 8);
                assert_eq!(values.align_bits, Some(numbers["vector_align"] * 8));
                assert_eq!(counter.size_bits, numbers["element_size"] * 8);
                assert_eq!(numbers["element_align"], 8);
            }
            reports.push(report);
        }
        assert_eq!(reports[0][0].size_bits, reports[1][0].size_bits);
        assert_eq!(reports[0][0].align_bits, reports[1][0].align_bits);
        let field = |variant: usize, path| {
            reports[variant]
                .iter()
                .find(|f| f.path == path)
                .unwrap()
                .offset_bits
        };
        assert_ne!(field(0, "record.counter"), field(1, "record.counter"));
        assert_ne!(field(0, "record.values"), field(1, "record.values"));
        assert_ne!(reports[0], reports[1]);
    }
    println!(
        "DIAGNOSTIC ONLY: no retention admission; allocation domain, pointer validity, element initialization, validity ranges and runtime interpretation remain unproven."
    );
}
