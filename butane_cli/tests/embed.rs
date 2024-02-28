use butane::_filenames::BUTANE_DIRNAME;

#[test]
fn working_dir_path() {
    let path = butane_cli::working_dir_path();
    assert!(path.ends_with("butane_cli/"));
}

#[test]
fn embed() {
    let example_dir = std::env::current_dir()
        .unwrap()
        .join("../examples/getting_started")
        .join(BUTANE_DIRNAME);
    assert!(example_dir.exists());
    butane_cli::embed(&example_dir).unwrap();
}
