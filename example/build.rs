fn main() {
    // This cleans the .butane/ directory which is generated when compiling the tests so that
    // the tests do not encounter side effects from previous test runs.
    // Currently the only way to remove stale items from the generated .butane/ directory is to
    // delete it before the code is compiled.
    // This means we can not rely on `butane clean` or the code behind it, because it hasnt
    // been compiled yet.
    let dir = ".butane/";
    println!("cargo:rerun-if-changed={dir}");
    if std::path::Path::new(&dir).is_dir() {
        std::fs::remove_dir_all(dir).unwrap();
    }
    let db = "db.sqlite";
    if std::path::Path::new(&db).is_dir() {
        std::fs::remove_file(db).unwrap();
    }
}
