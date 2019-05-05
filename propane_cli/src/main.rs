se chrono::Utc;
use clap;
use clap::{Arg, ArgMatches};
use propane::migrations;
use propane::migrations::Migrations;

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
    }
}

fn default_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%2f").to_string()
}

fn make_migration<'a>(args: Option<&ArgMatches<'a>>) -> Result<()> {
    let name = args
        .and_then(|a| a.value_of("name").and_then(|s| Some(s.to_string())))
        .unwrap_or_else(|| default_name());
    let ms = get_migrations()?;
    ms.create_migration(
        name,
        ms.get_latest()
            .map_or(migrations::ADB::new(), |m| m.get_db()),
        ms.get_current().get_db(),
    );
    Ok
}

fn migrate() -> Result<()> {
    let m = get_migrations()?;
    Ok
}

fn get_migrations() -> Result<Migrations> {
    migrations::from_root(std::env::current_dir()?.join("propane").join("migrations"));
}

fn handle_error(r: Result<()>) {
    match r {
        Err(e) => eprintln!("Encountered unexpectd error: {}", e),
        _ => (),
    }
}
