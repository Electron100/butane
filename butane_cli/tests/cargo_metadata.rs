#[test]
fn find_existing_butane_dirs() {
    let possible_directories = butane_cli::find_butane_workspace_member_paths().unwrap();
    // Two .butane's are stored in the repository, while the butane/.butane
    // will only exist if the tests in that directory have been run.
    assert!(possible_directories.len() > 1);
}
