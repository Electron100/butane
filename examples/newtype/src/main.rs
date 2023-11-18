use butane::db::{Connection, ConnectionSpec};
use butane::Error;

use newtype_example::{create_record, fetch_record, Patch};

type Result<T> = std::result::Result<T, Error>;

fn establish_connection() -> Result<Connection> {
    let mut cwd = std::env::current_dir()?;
    cwd.push(".butane");
    let spec = ConnectionSpec::load(cwd)?;
    let conn = butane::db::connect(&spec)?;
    Ok(conn)
}

fn main() -> Result<()> {
    let conn = establish_connection()?;
    let record = create_record(&conn, Patch::default());
    let record = fetch_record(&conn, &record.id);
    println!("{record:?}");
    Ok(())
}
