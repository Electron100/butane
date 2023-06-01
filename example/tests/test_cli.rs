use assert_cmd::Command;

#[test]
fn test_migrate_and_query() {
    let db = "db.sqlite";
    let connspec = ".butane/connection.json";

    // These files should have been removed by build.rs
    assert!(!std::path::Path::new(&db).exists());
    assert!(!std::path::Path::new(&connspec).exists());

    // This ensures the binary exists if `example` is the first project tested
    Command::new("cargo")
        .args(["build", "--workspace", "--bin=butane"])
        .assert()
        .success();

    Command::cargo_bin("butane")
        .unwrap()
        .args(["init", "sqlite", db])
        .assert()
        .success();

    // Verify the files have been created by "init", as they are needed by makemigration
    assert!(std::path::Path::new(&db).exists());
    assert!(std::path::Path::new(&connspec).exists());

    let result = Command::cargo_bin("butane")
        .unwrap()
        .args(["makemigration", "initial"])
        .assert()
        .success();

    println!(
        "stdout {}",
        String::from_utf8(result.get_output().stdout.clone()).unwrap()
    );
    println!(
        "stderr {}",
        String::from_utf8(result.get_output().stderr.clone()).unwrap()
    );
    assert!(result.get_output().stdout.starts_with(b"Created migration"));

    Command::cargo_bin("butane")
        .unwrap()
        .args(["migrate"])
        .assert()
        .success();

    Command::cargo_bin("example")
        .unwrap()
        .env("RUST_BACKTRACE", "1")
        .assert()
        .success();
}
