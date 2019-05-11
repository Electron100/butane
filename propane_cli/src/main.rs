use chrono::Utc;
use clap;
use clap::{Arg, ArgMatches};
use propane::migrations::Migrations;
use propane::{db, migrations};

type Result<T> = std::result::Result<T, failure::Error>;

fn main() {
    let args = clap::App::new("propane")
        .version(env!("CARGO_PKG_VERSION"))
        .author("James Oakley <james@electronstudio.org>")
        .about("Manages propane database migrations")
        .subcommand(
            clap::SubCommand::with_name("makemigration")
                .about("Used for configuration")
                .arg(Arg::with_name("name").help("Name to use for the migration")),
        )
        .subcommand(clap::SubCommand::with_name("migrate"))
        .get_matches();
    match args.subcommand() {
        ("makemigration", sub_args) => handle_error(make_migration(sub_args)),
        ("migrate", _) => handle_error(migrate()),
        (_, _) => eprintln!("Unknown command"),
    }
}

fn default_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%3f").to_string()
}

fn make_migration<'a>(args: Option<&ArgMatches<'a>>) -> Result<()> {
    let name = args
        .and_then(|a| a.value_of("name").and_then(|s| Some(s.to_string())))
        .unwrap_or_else(|| default_name());
    let ms = get_migrations()?;
    let m = ms.create_migration_sql(
        db::sqlite_backend(),
        &name,
        ms.get_latest(),
        &ms.get_current(),
    )?;
    match m {
        Some(m) => println!("Created migration {}", m.get_name()),
        None => println!("No changes to migrate"),
    }
    Ok(())
}

fn migrate() -> Result<()> {
    let m = get_migrations()?;
    //todo
    Ok(())
}

fn get_migrations() -> Result<Migrations> {
    Ok(migrations::from_root(
        std::env::current_dir()?.join("propane").join("migrations"),
    ))
}

fn handle_error(r: Result<()>) {
    match r {
        Err(e) => eprintln!("Encountered unexpectd error: {}", e),
        _ => (),
    }
}
