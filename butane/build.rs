fn main() {
    // This cleans the .butane/ directory which is generated when compiling the tests so that
    // the tests do not encounter side effects from previous test runs.
    // Currently the only way to remove stale items from the generated .butane/ directory is to
    // delete it before the code is compiled.
    // This means we can not rely on `butane clean` or the code behind it, because it hasnt
    // been compiled yet.
    let dir = ".butane/";
    println!("cargo:rerun-if-changed={dir}");
    if std::path::Path::new(&dir).exists() {
        match std::fs::remove_dir_all(dir) {
            Ok(_) => {
                // Re-create the directory. Only tests populate it and if it is left non-existent
                // Cargo will detect it as changed and a no-op build will not in fact no-op
                std::fs::create_dir(dir).unwrap();
            }
            Err(_) => eprintln!("Cannot delete .butane dir"),
        }
    }
}
