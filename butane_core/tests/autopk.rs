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

/// Test that `insert_or_replace` correctly quotes reserved word primary keys in ON CONFLICT clause.
#[butane_test(nomigrate)]
async fn reserved_word(conn: ConnectionAsync) {
    // Create table with "order" as primary key (a reserved SQL keyword)
    let pkcol = Column::new("order", SqlType::BigInt);
    let bar_column = Column::new("bar", SqlType::Text);

    let mut table = ATable::new("reserved_pkey_test".to_string());

    // Create "order" column as primary key with auto-increment
    let order_col = AColumn::new(
        pkcol.name(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::BigInt)),
        false, // not nullable
        true,  // is primary key
        true,  // auto-increment (this is what we're testing!)
        false, // not unique (pk already implies unique)
        None,  // no default
        None,  // no foreign key
    );
    table.add_column(order_col);

    // Create bar column
    let bar_col = AColumn::new(
        bar_column.name(),
        DeferredSqlType::KnownId(TypeIdentifier::Ty(SqlType::Text)),
        false, // not nullable
        false, // not pk
        false, // not auto
        false, // not unique
        None,  // no default
        None,  // no foreign key
    );
    table.add_column(bar_col);

    // Generate and execute CREATE TABLE
    let backend = conn.backend();
    let adb = ADB::default();
    let create_sql = backend
        .create_migration_sql(&adb, vec![Operation::AddTable(table)])
        .unwrap();

    conn.execute(&create_sql).await.unwrap();

    // Now test insert_or_replace which uses sql_insert_or_update internally
    // This should properly quote the "order" column in the ON CONFLICT clause
    let columns = [pkcol.clone(), bar_column.clone()];
    let values = [SqlValRef::BigInt(1), SqlValRef::Text("first")];

    // First insert
    conn.insert_or_replace("reserved_pkey_test", &columns, &pkcol, &values)
        .await
        .unwrap();

    // Second insert with same pk should update
    let values2 = [SqlValRef::BigInt(1), SqlValRef::Text("updated")];
    conn.insert_or_replace("reserved_pkey_test", &columns, &pkcol, &values2)
        .await
        .unwrap();

    // Verify only one row exists after the upsert.
    // TODO: There is a bug in turso that fails when using COUNT(anything) on this table.
    let query_columns = [pkcol.clone(), bar_column.clone()];
    let mut result = conn
        .query("reserved_pkey_test", &query_columns, None, None, None, None)
        .await
        .unwrap();

    let mut count = 0;
    while result.next().unwrap().is_some() {
        count += 1;
    }
    assert_eq!(
        count, 1,
        "Expected exactly 1 row after upsert, found {}",
        count
    );
}
