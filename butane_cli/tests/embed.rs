#[test]
fn working_dir_path() {
    let path = butane_cli::working_dir_path();
    assert!(path.ends_with(&"butane_cli/"));
}

#[test]
fn embed() {
    let example_dir = std::env::current_dir()
        .unwrap()
        .join("../examples/getting_started/.butane");
    assert!(example_dir.exists());
    butane_cli::embed(&example_dir).unwrap();
    for filename in ["../src/butane_migrations.rs", "clistate.json"] {
        let path = example_dir.join(filename);
        std::fs::remove_file(path).unwrap();
    }
}
