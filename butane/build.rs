fn main() {
    println!("cargo:rerun-if-changed=tests/");

    // Note that #[cfg(test)] and CARGO_CFG_TEST do not work in build.rs
    // See https://github.com/rust-lang/cargo/issues/4789
    #[cfg(feature = "_auto_delete_dot_butane")]
    {
        clean_dot_butane();
    }
}

/// This cleans the .butane/ directory which is generated when compiling the tests so that
/// the tests do not encounter side effects from previous test runs.
/// Currently the only way to remove stale items from the generated .butane/ directory is to
/// delete it before the code is compiled.
/// This means we can not rely on `butane clean` or the code behind it, because it hasnt
/// been compiled yet.
#[cfg(feature = "_auto_delete_dot_butane")]
fn clean_dot_butane() {
    let dir = ".butane/";
    if std::path::Path::new(&dir).is_dir() {
        println!("cargo:warning=Deleting .butane directory");
        if std::fs::remove_dir_all(dir).is_err() {
            println!("cargo:warning=Cannot delete .butane directory");
        }
    }
}
