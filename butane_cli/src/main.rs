use butane::migrations::{
    copy_migration, FsMigrations, MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut,
};
use butane::{db, db::Connection, migrations};
use chrono::Utc;
use clap::{Arg, ArgMatches};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, anyhow::Error>;

fn main() {
    let app = clap::App::new("butane")
        .version(env!("CARGO_PKG_VERSION"))
        .author("James Oakley <james@electronstudio.org>")
        .about("Manages butane database migrations")
        .subcommand(
            clap::SubCommand::with_name("init")
                .about("Initialize the database")
                .arg(
                    Arg::with_name("BACKEND")
                        .required(true)
                        .index(1)
                        .help("Database backend to use. 'sqlite' or 'pg'"),
                )
                .arg(
                    Arg::with_name("CONNECTION")
                        .required(true)
                        .index(2)
                        .help("Database connection string. Format depends on backend"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("makemigration")
                .about("Create a new migration")
                .arg(
                    Arg::with_name("NAME")
                        .required(true)
                        .index(1)
                        .help("Name to use for the migration"),
                ),
        )
        .subcommand(clap::SubCommand::with_name("migrate").about("Apply migrations"))
        .subcommand(clap::SubCommand::with_name("list").about("List migrations"))
        .subcommand(
            clap::SubCommand::with_name("embed").about("Embed migrations in the source code"),
        )
        .subcommand(
            clap::SubCommand::with_name("rollback")
                .about("Rollback migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration")
                .arg(
                    Arg::with_name("NAME")
                        .required(false)
                        .index(1)
                        .help("Migration to roll back to"),
                ),
        )
        .subcommand(
            clap::SubCommand::with_name("delete")
                .about("Delete a table")
                .setting(clap::AppSettings::ArgRequiredElseHelp)
                .subcommand(
                    clap::SubCommand::with_name("table")
                        .about("Delete a table")
                        .arg(
                            Arg::with_name("TABLE")
                                .required(true)
                                .index(1)
                                .help("Name of table to delete"),
                        ),
                ),
        )
        .setting(clap::AppSettings::ArgRequiredElseHelp);
    let args = app.get_matches();
    match args.subcommand() {
        ("init", sub_args) => handle_error(init(sub_args)),
        ("makemigration", sub_args) => handle_error(make_migration(sub_args)),
        ("migrate", _) => handle_error(migrate()),
        ("rollback", sub_args) => handle_error(rollback(sub_args)),
        ("embed", _) => handle_error(embed()),
        ("list", _) => handle_error(list_migrations()),
        ("delete", Some(sub_args)) => match sub_args.subcommand() {
            ("table", Some(sub_args2)) => {
                handle_error(delete_table(sub_args2.value_of("TABLE").unwrap()))
            }
            (_, _) => eprintln!("Unknown delete command. Try: delete table"),
        },
        (cmd, _) => eprintln!("Unknown command {}", cmd),
    }
}

#[derive(Serialize, Deserialize, Default)]
struct CliState {
    embedded: bool,
}
impl CliState {
    pub fn load() -> Result<Self> {
        let path = base_dir()?.join("clistate.json");
        let file = File::open(path);
        match file {
            Ok(file) => Ok(serde_json::from_reader(file)?),
            Err(_) => Ok(CliState::default()),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = base_dir()?.join("clistate.json");
        let file = File::create(path)?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}

fn default_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%3f").to_string()
}

fn init(args: Option<&ArgMatches>) -> Result<()> {
    let args = args.unwrap();
    let name = args.value_of("BACKEND").unwrap();
    let connstr = args.value_of("CONNECTION").unwrap();
    if db::get_backend(name).is_none() {
        eprintln!("Unknown backend {}", name);
        std::process::exit(1);
    };

    let spec = db::ConnectionSpec::new(name, connstr);
    db::connect(&spec)?; // ensure we can
    std::fs::create_dir_all(base_dir()?)?;
    spec.save(&base_dir()?)?;

    Ok(())
}

fn make_migration(args: Option<&ArgMatches>) -> Result<()> {
    let name_arg = args.map(|a| a.value_of("NAME")).flatten();
    let name = match name_arg {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let mut ms = get_migrations()?;
    if ms.all_migrations()?.iter().any(|m| m.name() == name) {
        eprintln!("Migration {} already exists", name);
        std::process::exit(1);
    }
    let spec = load_connspec()?;
    let backend = match db::get_backend(&spec.backend_name) {
        Some(backend) => backend,
        None => {
            eprintln!("Unknown backend {}", &spec.backend_name);
            std::process::exit(1);
        }
    };
    let created = ms.create_migration(&backend, &name, ms.latest().as_ref())?;
    if created {
        let cli_state = CliState::load()?;
        if cli_state.embedded {
            // Better include the new migration in the embedding
            embed()?;
        }
        println!("Created migration {}", name);
    } else {
        println!("No changes to migrate");
    }
    Ok(())
}

fn migrate() -> Result<()> {
    let spec = load_connspec()?;
    let mut conn = db::connect(&spec)?;
    let to_apply = get_migrations()?.unapplied_migrations(&conn)?;
    println!("{} migrations to apply", to_apply.len());
    for m in to_apply {
        println!("Applying migration {}", m.name());
        m.apply(&mut conn)?;
    }
    Ok(())
}

fn rollback(args: Option<&ArgMatches>) -> Result<()> {
    let spec = load_connspec()?;
    let conn = db::connect(&spec)?;

    match args.map(|a| a.value_of("NAME")).flatten() {
        Some(to) => rollback_to(conn, to),
        None => rollback_latest(conn),
    }
}

fn rollback_to(mut conn: Connection, to: &str) -> Result<()> {
    let ms = get_migrations()?;
    let to_migration = match ms.get_migration(to) {
        Some(m) => m,
        None => {
            eprintln!("No such migration!");
            std::process::exit(1);
        }
    };

    let to_unapply = ms.migrations_since(&to_migration)?;
    if to_unapply.is_empty() {
        eprintln!("That is the latest migration, not rolling back to anything. If you expected something to happen, try specifying the migration to rollback to.");
    }
    for m in to_unapply.into_iter().rev() {
        println!("Rolling back migration  {}", m.name());
        m.downgrade(&mut conn)?;
    }
    Ok(())
}

fn rollback_latest(mut conn: Connection) -> Result<()> {
    match get_migrations()?.latest() {
        Some(m) => {
            println!("Rolling back migration  {}", m.name());
            m.downgrade(&mut conn)?;
        }
        None => {
            eprintln!("No migrations applied!");
            std::process::exit(1)
        }
    };
    Ok(())
}

fn embed() -> Result<()> {
    let srcdir = std::env::current_dir()?.join("src");
    if !srcdir.exists() {
        eprintln!("src directory not found");
        std::process::exit(1);
    }
    let path = srcdir.join("butane_migrations.rs");

    let mut mem_ms = MemMigrations::new();
    for m in get_migrations()?.all_migrations()? {
        let mut new_m = mem_ms.new_migration(&m.name());
        copy_migration(&m, &mut new_m)?;
        mem_ms.add_migration(new_m)?;
    }
    let json = serde_json::to_string(&mem_ms)?;

    let src = format!(
        "
use butane::migrations::MemMigrations;
use std::result::Result;
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {{
    let json = r#\"{}\"#;
    MemMigrations::from_json(json)
}}",
        json
    );

    let mut f = std::fs::File::create(path)?;
    f.write_all(src.as_bytes())?;

    let mut cli_state = CliState::load()?;
    cli_state.embedded = true;
    cli_state.save()?;
    Ok(())
}

fn load_connspec() -> Result<db::ConnectionSpec> {
    match db::ConnectionSpec::load(&base_dir()?) {
        Ok(spec) => Ok(spec),
        Err(butane::Error::IO(_)) => {
            eprintln!("No Butane connection info found. Did you run butane init?");
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}

fn list_migrations() -> Result<()> {
    let spec = load_connspec()?;
    let conn = db::connect(&spec)?;
    let ms = get_migrations()?;
    let unapplied = ms.unapplied_migrations(&conn)?;
    let all = ms.all_migrations()?;
    for m in all {
        let m_state = match unapplied.contains(&m) {
            true => "not applied",
            false => "applied",
        };
        println!("Migration '{}' ({})", m.name(), m_state);
    }
    Ok(())
}

fn delete_table(name: &str) -> Result<()> {
    let mut ms = get_migrations()?;
    let current = ms.current();
    current.delete_table(name)?;
    Ok(())
}

fn get_migrations() -> Result<FsMigrations> {
    let root = base_dir()?.join("migrations");
    if !root.exists() {
        eprintln!("No butane migrations directory found. Add at least one model to your project and build.");
        std::process::exit(1);
    }
    Ok(migrations::from_root(root))
}

fn base_dir() -> Result<PathBuf> {
    std::env::current_dir()
        .map(|d| d.join(".butane"))
        .map_err(|e| e.into())
}

fn handle_error(r: Result<()>) {
    if let Err(e) = r {
        eprintln!("Encountered unexpected error: {}", e);
        std::process::exit(1);
    }
}
