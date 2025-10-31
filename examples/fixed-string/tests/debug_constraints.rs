use butane::db::Connection;
use butane::migrations::Migrations;
use butane_test_helper::*;
use butane_test_macros::butane_test;

#[butane_test(sync, nomigrate, pg)]
fn debug_constraint_names(mut connection: Connection) {
    // Migrate forward to create the tables and constraints
    let base_dir = std::path::PathBuf::from(".butane");
    let migrations = butane_cli::get_migrations(&base_dir).unwrap();
    migrations.migrate(&mut connection).unwrap();

    // Test constraint name generation manually
    println!("Testing constraint name generation...");

    // Try with lowercase constraint name (PostgreSQL lowercases unquoted identifiers)
    let drop_result0 = connection.execute("ALTER TABLE \"Order\" DROP CONSTRAINT order_user_fkey;");
    match drop_result0 {
        Ok(_) => println!("Successfully dropped order_user_fkey (lowercase)"),
        Err(e) => println!("Failed to drop order_user_fkey (lowercase): {}", e),
    }

    // Try to drop the constraint with the exact name from the down migration
    let drop_result1 = connection.execute("ALTER TABLE \"Order\" DROP CONSTRAINT Order_user_fkey;");
    match drop_result1 {
        Ok(_) => println!("Successfully dropped Order_user_fkey"),
        Err(e) => println!("Failed to drop Order_user_fkey: {}", e),
    }

    // Try with quoted constraint name
    let drop_result2 =
        connection.execute("ALTER TABLE \"Order\" DROP CONSTRAINT \"Order_user_fkey\";");
    match drop_result2 {
        Ok(_) => println!("Successfully dropped \"Order_user_fkey\""),
        Err(e) => println!("Failed to drop \"Order_user_fkey\": {}", e),
    }

    // Try to find the actual constraint name by attempting to recreate it and see what error we get
    let recreate_result = connection
        .execute("ALTER TABLE \"Order\" ADD FOREIGN KEY (\"user\") REFERENCES \"User\"(\"id\");");
    match recreate_result {
        Ok(_) => println!("Successfully recreated constraint (shouldn't happen)"),
        Err(e) => println!("Failed to recreate constraint (expected): {}", e),
    } // Also check the down migration SQL to see what it's trying to drop
    println!("\nDown migration attempts to drop:");
    println!("ALTER TABLE \"Order\" DROP CONSTRAINT Order_user_fkey;");
    println!("ALTER TABLE \"Order\" DROP CONSTRAINT Order_product_fkey;");
}
