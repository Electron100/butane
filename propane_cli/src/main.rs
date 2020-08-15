use chrono::Utc;
use clap;
use clap::{Arg, ArgMatches};
use propane::migrations::{
    copy_migration, FsMigrations, MemMigrations, Migration, Migrations, MigrationsMut,
};
use propane::{db, migrations};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, failure::Error>;

fn main() {
    let args = clap::App::new("propane")
        .version(env!("CARGO_PKG_VERSION"))
        .author("James Oakley <james@electronstudio.org>")
        .about("Manages propane database migrations")
        .subcommand(
            clap::SubCommand::with_name("init")
                .about("Initialize the database")
                .arg(
                    Arg::with_name("BACKEND")
                        .required(true)
                        .index(1)
                        .help("Database backend to use. Currently only 'sqlite' is supported."),
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
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .get_matches();
    match args.subcommand() {
        ("init", sub_args) => handle_error(init(sub_args)),
        ("makemigration", sub_args) => handle_error(make_migration(sub_args)),
        ("migrate", _) => handle_error(migrate()),
        ("embed", _) => handle_error(embed()),
        ("list", _) => handle_error(list_migrations()),
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

fn init<'a>(args: Option<&ArgMatches<'a>>) -> Result<()> {
    let args = args.unwrap();
    let name = args.value_of("BACKEND").unwrap();
    let connstr = args.value_of("CONNECTION").unwrap();
    if db::get_backend(name).is_none() {
        eprintln!("Unknown backend {}", name);
        std::process::exit(1);
    };

    let spec = db::ConnectionSpec::new(name, connstr);
    db::connect(&spec)?; // ensure we can
    std::fs::create_dir(base_dir()?)?;
    spec.save(&base_dir()?)?;

    Ok(())
}

fn make_migration<'a>(args: Option<&ArgMatches<'a>>) -> Result<()> {
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
    let created = ms.create_migration(
        &db::sqlite::SQLiteBackend::new(),
        &name,
        ms.latest().as_ref(),
    )?;
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
    let spec = db::ConnectionSpec::load(&base_dir()?)?;
    let mut conn = db::connect(&spec)?;
    let to_apply = get_migrations()?.unapplied_migrations(&conn)?;
    println!("{} migrations to apply", to_apply.len());
    for m in to_apply {
        println!("Applying migration {}", m.name());
        m.apply(&mut conn)?;
    }
    Ok(())
}

fn embed() -> Result<()> {
    let srcdir = std::env::current_dir()?.join("src");
    if !srcdir.exists() {
        eprintln!("src directory not found");
        std::process::exit(1);
    }
    let path = srcdir.join("propane_migrations.rs");

    let mut mem_ms = MemMigrations::new();
    for m in get_migrations()?.all_migrations()? {
        let mut new_m = mem_ms.new_migration(&m.name());
        copy_migration(&m, &mut new_m)?;
        mem_ms.add_migration(new_m)?;
    }
    let json = serde_json::to_string(&mem_ms)?;

    let src = format!(
        "
use propane::migrations::MemMigrations;
use std::result::Result;
pub fn get_migrations() -> Result<MemMigrations, propane::Error> {{
    let json = r#\"{}\"#;
    MemMigrations::from_json(json)
}}",
        json
    );

    let mut f = std::fs::File::create(path)?;
    f.write_all(format!("{}", src).as_bytes())?;

    let mut cli_state = CliState::load()?;
    cli_state.embedded = true;
    cli_state.save()?;
    Ok(())
}

fn list_migrations() -> Result<()> {
    let spec = db::ConnectionSpec::load(&base_dir()?)?;
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

fn get_migrations() -> Result<FsMigrations> {
    let root = base_dir()?.join("migrations");
    if !root.exists() {
        eprintln!("No propane migrations directory found. Add at least one model to your project and build.");
        std::process::exit(1);
    }
    Ok(migrations::from_root(root))
}

fn base_dir() -> Result<PathBuf> {
    std::env::current_dir()
        .map(|d| d.join("propane"))
        .map_err(|e| e.into())
}

fn handle_error(r: Result<()>) {
    if let Err(e) = r {
        eprintln!("Encountered unexpected error: {}", e);
        std::process::exit(1);
    }
}
