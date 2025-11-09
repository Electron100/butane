//! Harness for using trybuild to test macros.

#[test]
fn compile_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/trybuild/pass/*.rs");
    t.compile_fail("tests/trybuild/fail/*.rs");

    // These demonstrate cases that should fail but currently pass.
    t.pass("tests/trybuild/should-fail/*.rs");
}
