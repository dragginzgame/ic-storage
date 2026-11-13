//! Compile-time refusal of invalid collection/lease integrations.
#[test]
fn capability_and_dto_boundaries_are_compile_time_enforced() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/open_during_admission.rs");
    tests.compile_fail("tests/ui/clone_allocation_lease.rs");
}
