#![doc(hidden)]
//! This library is not stable, and usage is strongly discouraged.
//!
//! It is intended only to assist developing the CLI.
//! Usage of this library is strongly discouraged unless you expect & accept
//! breakages in the future.
//! Backwards compatibility of the library will not even be considered, as the
//! only objective of the crate is to provide a stable CLI.
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use butane::migrations::{
    copy_migration, FsMigrations, MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut,
};
use butane::query::BoolExpr;
use butane::{db, db::Connection, db::ConnectionMethods, migrations};
use chrono::Utc;
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CliState {
    embedded: bool,
}
impl CliState {
    pub fn load(base_dir: &Path) -> Result<Self> {
        let path = base_dir.join("clistate.json");
        let file = File::open(path);
        match file {
            Ok(file) => Ok(serde_json::from_reader(file)?),
            Err(_) => Ok(CliState::default()),
        }
    }

    pub fn save(&self, base_dir: &Path) -> Result<()> {
        let path = base_dir.join("clistate.json");
        let file = File::create(path)?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}

pub fn default_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%3f").to_string()
}

pub fn init(base_dir: &PathBuf, name: &str, connstr: &str) -> Result<()> {
    if db::get_backend(name).is_none() {
        eprintln!("Unknown backend {name}");
        std::process::exit(1);
    };

    let spec = db::ConnectionSpec::new(name, connstr);
    db::connect(&spec)?; // ensure we can
    std::fs::create_dir_all(base_dir)?;
    spec.save(base_dir)?;

    Ok(())
}

pub fn make_migration(base_dir: &PathBuf, name: Option<&String>) -> Result<()> {
    let name = match name {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let mut ms = get_migrations(base_dir)?;
    if ms.all_migrations()?.iter().any(|m| m.name() == name) {
        eprintln!("Migration {name} already exists");
        std::process::exit(1);
    }
    let spec = load_connspec(base_dir)?;
    let backend = spec.get_backend()?;
    let created = ms.create_migration(&backend, &name, ms.latest().as_ref())?;
    if created {
        let cli_state = CliState::load(base_dir)?;
        if cli_state.embedded {
            // Better include the new migration in the embedding
            embed(base_dir)?;
        }
        println!("Created migration {name}");
    } else {
        println!("No changes to migrate");
    }
    Ok(())
}

pub fn migrate(base_dir: &PathBuf) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let mut conn = db::connect(&spec)?;
    let to_apply = get_migrations(base_dir)?.unapplied_migrations(&conn)?;
    println!("{} migrations to apply", to_apply.len());
    for m in to_apply {
        println!("Applying migration {}", m.name());
        m.apply(&mut conn)?;
    }
    Ok(())
}

pub fn rollback_to(base_dir: &Path, mut conn: Connection, to: &str) -> Result<()> {
    let ms = get_migrations(base_dir)?;
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
        println!("Rolling back migration {}", m.name());
        m.downgrade(&mut conn)?;
    }
    Ok(())
}

pub fn rollback_latest(base_dir: &Path, mut conn: Connection) -> Result<()> {
    match get_migrations(base_dir)?.latest() {
        Some(m) => {
            println!("Rolling back migration {}", m.name());
            m.downgrade(&mut conn)?;
        }
        None => {
            eprintln!("No migrations applied!");
            std::process::exit(1)
        }
    };
    Ok(())
}

pub fn embed(base_dir: &Path) -> Result<()> {
    let srcdir = base_dir.join("../src");
    if !srcdir.exists() {
        eprintln!("src directory not found");
        std::process::exit(1);
    }
    let path = srcdir.join("butane_migrations.rs");

    let mut mem_ms = MemMigrations::new();
    let migrations = get_migrations(base_dir)?;
    let migration_list = migrations.all_migrations()?;
    for m in migration_list {
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

    let mut cli_state = CliState::load(base_dir)?;
    cli_state.embedded = true;
    cli_state.save(base_dir)?;
    Ok(())
}

pub fn load_connspec(base_dir: &PathBuf) -> Result<db::ConnectionSpec> {
    match db::ConnectionSpec::load(base_dir) {
        Ok(spec) => Ok(spec),
        Err(butane::Error::IO(_)) => {
            eprintln!("No Butane connection info found. Did you run butane init?");
            std::process::exit(1);
        }
        Err(e) => Err(e.into()),
    }
}

pub fn list_migrations(base_dir: &PathBuf) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let conn = db::connect(&spec)?;
    let ms = get_migrations(base_dir)?;
    let unapplied = ms.unapplied_migrations(&conn)?;
    let all = ms.all_migrations()?;
    for m in all {
        let m_state = if unapplied.contains(&m) {
            "not applied"
        } else {
            "applied"
        };
        println!("Migration '{}' ({})", m.name(), m_state);
    }
    Ok(())
}

pub fn collapse_migrations(base_dir: &PathBuf, new_initial_name: Option<&String>) -> Result<()> {
    let name = match new_initial_name {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let spec = load_connspec(base_dir)?;
    let backend = spec.get_backend()?;
    let conn = db::connect(&spec)?;
    let mut ms = get_migrations(base_dir)?;
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
    let cli_state = CliState::load(base_dir)?;
    if cli_state.embedded {
        // Update the embedding
        embed(base_dir)?;
    }
    println!("Collapsed all changes into new single migration '{name}'");
    Ok(())
}

pub fn delete_table(base_dir: &Path, name: &str) -> Result<()> {
    let mut ms = get_migrations(base_dir)?;
    let current = ms.current();
    current.delete_table(name)?;
    Ok(())
}

pub fn clear_data(base_dir: &PathBuf) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let conn = db::connect(&spec)?;
    let latest = match get_migrations(base_dir)?.last_applied_migration(&conn)? {
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

pub fn clean(base_dir: &Path) -> Result<()> {
    get_migrations(base_dir)?.clear_current()?;
    Ok(())
}

pub fn get_migrations(base_dir: &Path) -> Result<FsMigrations> {
    let root = base_dir.join("migrations");
    if !root.exists() {
        eprintln!("No butane migrations directory found. Add at least one model to your project and build.");
        std::process::exit(1);
    }
    Ok(migrations::from_root(root))
}

pub fn base_dir() -> Result<PathBuf> {
    std::env::current_dir()
        .map(|d| d.join(".butane"))
        .map_err(|e| e.into())
}

pub fn handle_error(r: Result<()>) {
    if let Err(e) = r {
        eprintln!("Encountered unexpected error: {e}");
        std::process::exit(1);
    }
}
