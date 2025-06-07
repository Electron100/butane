use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use butane_test_helper::{pg_tmp_server_create, PgServerOptions};

fn main() {
    // Start the PostgreSQL server
    let server = pg_tmp_server_create(PgServerOptions {
        port: Some(5432),
        ..Default::default()
    })
    .unwrap();
    // Print the connection string
    println!(
        "Running temporary PostgreSQL server\nUnix socket dir: {}\nUser: postgres",
        server.sockdir.path().display()
    );

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    println!("Waiting for Ctrl-C...");
    while running.load(Ordering::SeqCst) {}
    println!("Exiting...");
}
