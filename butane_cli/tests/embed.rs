#[test]
fn working_dir_path() {
    let path = butane_cli::working_dir_path();
    assert!(path.ends_with("butane_cli/"));
}

#[test]
fn embed() {
    let example_dir = std::env::current_dir()
        .unwrap()
        .join("../examples/getting_started/.butane");
    assert!(example_dir.is_dir());
    butane_cli::embed(&example_dir).unwrap();
}
