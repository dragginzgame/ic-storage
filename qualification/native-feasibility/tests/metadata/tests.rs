use super::inspect;

// Independently authored metadata grammar fixture, not Rust executable source.
const BASIC: &str = r#"
!0 = !DICompositeType(tag: DW_TAG_structure_type, name: "LayoutProbeRecord", size: 64, align: 64, elements: !1)
!1 = !{!2}
!2 = !DIDerivedType(tag: DW_TAG_member, name: "counter", scope: !0, baseType: !3, size: 64, align: 64)
!3 = !DIBasicType(name: "u64", size: 64, encoding: DW_ATE_unsigned)
"#;

#[test]
fn scalar_alignment_remains_unknown_when_absent() {
    let report = inspect(BASIC).unwrap();
    assert_eq!(report.len(), 2);
    assert_eq!(report[1].offset_bits, 0);
    assert_eq!(report[1].align_bits, None);
}

#[test]
fn missing_metadata_and_missing_references_refuse() {
    assert!(inspect("").is_err());
    assert!(inspect(&BASIC.replace("baseType: !3", "baseType: !99")).is_err());
    assert!(inspect(&BASIC.replace("elements: !1", "elements: !99")).is_err());
}

#[test]
fn duplicate_roots_nodes_fields_and_attributes_refuse() {
    assert!(inspect(&format!("{BASIC}\n!4 = !DICompositeType(tag: DW_TAG_structure_type, name: \"LayoutProbeRecord\", size: 64, align: 64, elements: !1)")).is_err());
    assert!(
        inspect(&format!(
            "{BASIC}\n!3 = !DIBasicType(name: \"u64\", size: 64)"
        ))
        .is_err()
    );
    assert!(inspect(&BASIC.replace("!{!2}", "!{!2, !2}")).is_err());
    assert!(
        inspect(&BASIC.replace("size: 64, align: 64", "size: 64, size: 64, align: 64")).is_err()
    );
}

#[test]
fn truncated_unsupported_and_inconsistent_shapes_refuse() {
    assert!(inspect(&BASIC.replace("elements: !1)", "elements: !1")).is_err());
    assert!(inspect(&BASIC.replace("DW_TAG_structure_type", "DW_TAG_union_type")).is_err());
    assert!(inspect(&BASIC.replace("scope: !0", "scope: !3")).is_err());
    assert!(inspect(&BASIC.replace("size: 64, encoding", "size: 32, encoding")).is_err());
    assert!(inspect(&BASIC.replace("scope: !0,", "scope: !0, offset: 64,")).is_err());
    assert!(inspect(&BASIC.replace("align: 64, elements", "elements")).is_err());
    assert!(inspect(&BASIC.replace("elements: !1", "flags: DIFlagFwdDecl, elements: !1")).is_err());
    assert!(inspect(&BASIC.replace("!{!2}", "!{!2, }")).is_err());
}

#[test]
fn pointer_requires_a_target_but_does_not_claim_allocation_metadata() {
    let source = BASIC.replace(
        "!DIBasicType(name: \"u64\", size: 64, encoding: DW_ATE_unsigned)",
        "!DIDerivedType(tag: DW_TAG_pointer_type, name: \"*const u8\", size: 64, align: 64, baseType: !4)",
    ) + "\n!4 = !DIBasicType(name: \"u8\", size: 8, encoding: DW_ATE_unsigned)";
    assert_eq!(inspect(&source).unwrap()[1].kind, "pointer");
    assert!(inspect(&source.replace("baseType: !4", "baseType: !99")).is_err());
}

#[test]
fn by_value_recursion_refuses() {
    assert!(inspect(&BASIC.replace("baseType: !3", "baseType: !0")).is_err());
}
