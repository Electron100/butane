use butane::migrations::{
    copy_migration, FsMigrations, MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut,
};
use butane::query::BoolExpr;
use butane::{db, db::Connection, db::ConnectionMethods, migrations};
use chrono::Utc;
use clap::{Arg, ArgMatches};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, anyhow::Error>;

fn main() {
    let app = clap::Command::new("butane")
        .version(env!("CARGO_PKG_VERSION"))
        .author("James Oakley <james@electronstudio.org>")
        .about("Manages butane database migrations")
        .max_term_width(80)
        .subcommand(
            clap::Command::new("init")
                .about("Initialize the database")
                .arg(
                    Arg::new("BACKEND")
                        .required(true)
                        .index(1)
                        .help("Database backend to use. 'sqlite' or 'pg'"),
                )
                .arg(
                    Arg::new("CONNECTION")
                        .required(true)
                        .index(2)
                        .help("Database connection string. Format depends on backend"),
                ),
        )
        .subcommand(
            clap::Command::new("makemigration")
                .about("Create a new migration")
                .arg(
                    Arg::new("NAME")
                        .required(true)
                        .index(1)
                        .help("Name to use for the migration"),
                ),
        )
        .subcommand(clap::Command::new("migrate").about("Apply migrations"))
        .subcommand(clap::Command::new("list").about("List migrations"))
				.subcommand(clap::Command::new("collapse").about("Replace all migrations with a single migration representing the current model state.").arg(
                    Arg::new("NAME")
                        .required(true)
                        .index(1)
                        .help("Name to use for the new migration"),
                ))
        .subcommand(
            clap::Command::new("embed").about("Embed migrations in the source code"),
        )
        .subcommand(
            clap::Command::new("rollback")
                .about("Rollback migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration")
                .arg(
                    Arg::new("NAME")
                        .required(false)
                        .index(1)
                        .help("Migration to roll back to"),
                ),
        )
        .subcommand(
						clap::Command::new("clear")
								.arg_required_else_help(true)
								.about("Clear data")
								.subcommand(clap::Command::new("data")
														.about("Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted")))
        .subcommand(
            clap::Command::new("delete")
                .about("Delete a table")
                .arg_required_else_help(true)
                .subcommand(
                    clap::Command::new("table")
                        .about("Delete a table. Deleting a model in code does not currently lead to deletion of the table.")
                        .arg(
                            Arg::new("TABLE")
                                .required(true)
                                .index(1)
                                .help("Name of table to delete"),
                        ),
                ),
        )
				.subcommand(
						clap::Command::new("clean")
								.about("Clean current migration state. Deletes the current migration working state which is generated on each build. This can be used as a workaround to remove stale tables from the schema, as Butane does not currently auto-detect model removals. The next build will recreate with only tables for the extant models."))
                                .arg_required_else_help(true);
    let args = app.get_matches();
    match args.subcommand() {
        Some(("init", sub_args)) => handle_error(init(Some(sub_args))),
        Some(("makemigration", sub_args)) => handle_error(make_migration(Some(sub_args))),
        Some(("migrate", _)) => handle_error(migrate()),
        Some(("rollback", sub_args)) => handle_error(rollback(Some(sub_args))),
        Some(("embed", _)) => handle_error(embed()),
        Some(("list", _)) => handle_error(list_migrations()),
        Some(("collapse", sub_args)) => handle_error(collapse_migrations(sub_args.get_one("NAME"))),
        Some(("clear", sub_args)) => match sub_args.subcommand() {
            Some(("data", _)) => handle_error(clear_data()),
            _ => eprintln!("Unknown clear command. Try: clear data"),
        },
        Some(("delete", sub_args)) => match sub_args.subcommand() {
            Some(("table", sub_args2)) => {
                handle_error(delete_table(sub_args2.get_one::<&str>("TABLE").unwrap()))
            }
            _ => eprintln!("Unknown delete command. Try: delete table"),
        },
        Some(("clean", _)) => handle_error(clean()),
        Some((cmd, _)) => eprintln!("Unknown command {cmd}"),
        None => eprintln!("Unknown command"),
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
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
    let name: &String = args.get_one("BACKEND").unwrap();
    let connstr: &String = args.get_one("CONNECTION").unwrap();
    if db::get_backend(name).is_none() {
        eprintln!("Unknown backend {name}");
        std::process::exit(1);
    };

    let spec = db::ConnectionSpec::new(name, connstr);
    db::connect(&spec)?; // ensure we can
    std::fs::create_dir_all(base_dir()?)?;
    spec.save(&base_dir()?)?;

    Ok(())
}

fn make_migration(args: Option<&ArgMatches>) -> Result<()> {
    let name_arg = args.and_then(|a| a.get_one::<String>("NAME"));
    let name = match name_arg {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let mut ms = get_migrations()?;
    if ms.all_migrations()?.iter().any(|m| m.name() == name) {
        eprintln!("Migration {name} already exists");
        std::process::exit(1);
    }
    let spec = load_connspec()?;
    let backend = spec.get_backend()?;
    let created = ms.create_migration(&backend, &name, ms.latest().as_ref())?;
    if created {
        let cli_state = CliState::load()?;
        if cli_state.embedded {
            // Better include the new migration in the embedding
            embed()?;
        }
        println!("Created migration {name}");
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

    match args.and_then(|a| a.get_one::<String>("NAME")) {
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
    let json = r#\"{json}\"#;
    MemMigrations::from_json(json)
}}"
    );

    let mut f = std::fs::File::create(path)?;
    f.write_all(src.as_bytes())?;

    let mut cli_state = CliState::load()?;
    cli_state.embedded = true;
    cli_state.save()?;
    Ok(())
}

fn load_connspec() -> Result<db::ConnectionSpec> {
    match db::ConnectionSpec::load(base_dir()?) {
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

fn collapse_migrations(new_initial_name: Option<&String>) -> Result<()> {
    let name = match new_initial_name {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let spec = load_connspec()?;
    let backend = spec.get_backend()?;
    let conn = db::connect(&spec)?;
    let mut ms = get_migrations()?;
    let latest = ms.last_applied_migration(&conn)?;
    if latest.is_none() {
        eprintln!("There are no migrations to collapse");
        std::process::exit(1);
    }
    let latest_db = latest.unwrap().db()?;
    ms.clear_migrations(&conn)?;
    ms.create_migration_to(&backend, &name, None, latest_db)?;
    let new_migration = ms.latest().unwrap();
    new_migration.mark_applied(&conn)?;
    let cli_state = CliState::load()?;
    if cli_state.embedded {
        // Update the embedding
        embed()?;
    }
    println!("Collapsed all changes into new single migration '{name}'");
    Ok(())
}

fn delete_table(name: &str) -> Result<()> {
    let mut ms = get_migrations()?;
    let current = ms.current();
    current.delete_table(name)?;
    Ok(())
}

fn clear_data() -> Result<()> {
    let spec = load_connspec()?;
    let conn = db::connect(&spec)?;
    let latest = match get_migrations()?.last_applied_migration(&conn)? {
        Some(m) => m,
        None => {
            eprintln!("No migrations have been applied, so no data is recognized.");
            std::process::exit(1);
        }
    };
    for table in latest.db()?.tables() {
        println!("Deleting data from {}", &table.name);
        conn.delete_where(&table.name, BoolExpr::True)?;
    }
    Ok(())
}

fn clean() -> Result<()> {
    get_migrations()?.clear_current()?;
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
        eprintln!("Encountered unexpected error: {e}");
        std::process::exit(1);
    }
}
