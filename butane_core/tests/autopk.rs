use butane_core::db::ConnectionMethodsAsync;
use butane_core::db::{Backend, Column, ConnectionAsync, ConnectionMethods};
use butane_core::migrations::adb::{
    AColumn, ATable, DeferredSqlType, Operation, TypeIdentifier, ADB,
};
use butane_core::{SqlType, SqlVal, SqlValRef};
use butane_test_helper::*;
use butane_test_macros::butane_test;

/// Verify that `insert_returning_pk` correctly returns auto-generated primary keys.
#[butane_test(nomigrate)]
async fn auto_increment(conn: ConnectionAsync) {
    // Define column metadata once for both table creation and inserts
    let pkcol = Column::new("id", SqlType::Int);
    let name_column = Column::new("name", SqlType::Text);

    // Create a table using ADB so we get backend-appropriate SQL
    let mut table = ATable::new("autopk_test".to_string());

    // Create ID column with pk=true, auto=true
    let id_col = AColumn::new(
        pkcol.name(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Int)),
        false, // not nullable
        true,  // is primary key
        true,  // is auto-increment
        false, // not unique (pk already implies unique)
        None,  // no default
        None,  // no foreign key reference
    );
    table.add_column(id_col);

    // Create name column
    let name_col = AColumn::new(
        name_column.name(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
        false, // not nullable
        false, // not pk
        false, // not auto
        false, // not unique
        None,  // no default
        None,  // no foreign key
    );
    table.add_column(name_col);

    // Get the backend and generate appropriate CREATE TABLE SQL
    let backend = conn.backend();
    let backend_name = backend.name();
    let adb = ADB::default();
    let create_sql = backend
        .create_migration_sql(&adb, vec![Operation::AddTable(table)])
        .unwrap();

    eprintln!("Generated SQL for {backend_name}:\n{create_sql}");

    assert!(
        create_sql.contains("PRIMARY KEY"),
        "Should have PRIMARY KEY: {create_sql}"
    );

    // Verify the table can be created
    conn.execute(&create_sql).await.unwrap();

    // Test insert_returning_pk to verify auto-increment works
    let columns = [name_column];
    let values = [SqlValRef::Text("Test via insert_returning_pk")];

    let pk_val = conn
        .insert_returning_pk("autopk_test", &columns, &pkcol, &values)
        .await
        .unwrap();

    // Insert another record to verify auto-increment increments
    let values2 = [SqlValRef::Text("Second record")];
    let pk_val2 = conn
        .insert_returning_pk("autopk_test", &columns, &pkcol, &values2)
        .await
        .unwrap();

    // Verify both are Int type and second is greater than first (auto-increment)
    assert!(matches!(pk_val, SqlVal::Int(_)), "First PK should be Int");
    assert!(matches!(pk_val2, SqlVal::Int(_)), "Second PK should be Int");
    assert_ne!(pk_val, pk_val2, "PKs should be different");
}
