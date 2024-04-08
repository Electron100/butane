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

use butane::migrations::adb;
use butane::migrations::adb::{diff, AColumn, ARef, Operation, ADB};
use butane::migrations::{
    copy_migration, FsMigrations, MemMigrations, Migration, MigrationMut, Migrations, MigrationsMut,
};
use butane::query::BoolExpr;
use butane::{db, db::Connection, db::ConnectionMethods, migrations};
use cargo_metadata::MetadataCommand;
use chrono::Utc;
use nonempty::NonEmpty;
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
        let mut file = File::create(path)?;
        let mut contents = serde_json::to_string_pretty(self)?;
        contents.push('\n');
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}

pub fn default_name() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%3f").to_string()
}

pub async fn init(base_dir: &PathBuf, name: &str, connstr: &str, connect: bool) -> Result<()> {
    if db::get_backend(name).is_none() {
        eprintln!("Unknown backend {name}");
        std::process::exit(1);
    };

    let spec = db::ConnectionSpec::new(name, connstr);
    if connect {
        db::connect(&spec).await?;
    }
    std::fs::create_dir_all(base_dir)?;
    spec.save(base_dir)?;

    Ok(())
}

/// Make a migration.
/// The backends are selected from the existing migrations, or the initialised connection.
pub fn make_migration(base_dir: &Path, name: Option<&String>) -> Result<()> {
    let name = match name {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };
    let mut ms = get_migrations(base_dir)?;
    if ms.all_migrations()?.iter().any(|m| m.name() == name) {
        eprintln!("Migration {name} already exists");
        std::process::exit(1);
    }
    let backends = load_backends(base_dir)?;

    let created = ms.create_migration(&backends, &name, ms.latest().as_ref())?;
    if created {
        update_embedded(base_dir)?;
        println!("Created migration {name}");
    } else {
        println!("No changes to migrate");
    }
    Ok(())
}

/// Print a description of a column change indented by two spaces.
pub fn print_column_diff(old: &AColumn, new: &AColumn) -> Result<()> {
    if old.typeid()? != new.typeid()? {
        println!("  type: {:?} -> {:?}", old.typeid()?, new.typeid()?);
    }
    if old.is_pk() != new.is_pk() {
        println!("  pk: {} -> {}", old.is_pk(), new.is_pk());
    }
    if old.is_auto() != new.is_auto() {
        println!("  auto: {} -> {}", old.is_auto(), new.is_auto());
    }
    if old.nullable() != new.nullable() {
        println!("  nullable: {} -> {}", old.nullable(), new.nullable());
    }
    if old.unique() != new.unique() {
        println!("  unique: {} -> {}", old.unique(), new.unique());
    }
    if old.default() != new.default() {
        println!("  default: {:?} -> {:?}", old.default(), new.default());
    }
    if old.reference() != new.reference() {
        let old = match old.reference() {
            Some(ARef::Literal(reference)) => {
                format!("{}.{}", reference.table_name(), reference.column_name())
            }
            None => "None".to_string(),
            Some(ARef::Deferred(_)) => return Err(anyhow::anyhow!("ADB failed to resolve ARef.")),
        };
        let new = match new.reference() {
            Some(ARef::Literal(reference)) => {
                format!("{}.{}", reference.table_name(), reference.column_name())
            }
            None => "None".to_string(),
            Some(ARef::Deferred(_)) => return Err(anyhow::anyhow!("ADB failed to resolve ARef.")),
        };
        println!("  references: {} -> {}", old, new);
    }
    Ok(())
}

/// Print description of a list of [`Operation`].
pub fn print_ops(ops: Vec<Operation>) -> Result<()> {
    if ops.is_empty() {
        println!("No changes");
        return Ok(());
    }
    for op in &ops {
        use Operation::*;
        match op {
            AddTable(table) | AddTableIfNotExists(table) => {
                println!("New table {}", table.name);
                for column in &table.columns {
                    println!("  {}: {:?}", column.name(), column.typeid()?);
                }
            }
            AddTableConstraints(_) => {}
            RemoveTable(name) => {
                println!("Remove table {}", name);
            }
            AddColumn(table_name, column) => {
                println!(
                    "New column {table_name}.{}: {:?}",
                    column.name(),
                    column.typeid()?
                );
            }
            RemoveColumn(table_name, column_name) => {
                println!("Remove column {table_name}.{column_name}");
            }
            ChangeColumn(table_name, old, new) => {
                let column_name = old.name();
                // Rename currently isn't supported.
                // https://github.com/Electron100/butane/issues/89
                assert_eq!(column_name, new.name());
                println!("Change column {}.{column_name}", table_name);
                print_column_diff(old, new)?;
            }
        }
    }
    Ok(())
}

/// Print a description of the current migration.
pub fn describe_current_migration(base_dir: &Path) -> Result<()> {
    let mut ms = get_migrations(base_dir)?;
    let to_db = ms.current().db()?;
    let from_db = if let Some(latest) = ms.latest() {
        latest.db()?
    } else {
        ADB::new()
    };
    print_ops(diff(&from_db, &to_db))?;
    Ok(())
}

/// Describe a migration.
/// Use name "current" to describe the changes that have been made in the code
/// and will be included when a new migration is created.
pub fn describe_migration(base_dir: &Path, name: &String) -> Result<()> {
    if name == "current" {
        return describe_current_migration(base_dir);
    }
    let ms = get_migrations(base_dir)?;
    let migration = match ms.get_migration(name) {
        Some(m) => m,
        None => {
            eprintln!("No such migration!");
            std::process::exit(1);
        }
    };
    let to_db = migration.db()?;
    let from_db = match migration.migration_from()? {
        None => ADB::new(),
        Some(from_migration) => {
            let from = ms
                .get_migration(&from_migration)
                .expect("Migration should exist");
            from.db()?
        }
    };
    print_ops(diff(&from_db, &to_db))?;
    Ok(())
}

/// Detach the latest migration from the list of migrations,
/// leaving the migration on the filesystem.
pub async fn detach_latest_migration(base_dir: &PathBuf) -> Result<()> {
    let mut ms = get_migrations(base_dir)?;
    let all_migrations = ms.all_migrations().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });
    let initial_migration = all_migrations.first().unwrap_or_else(|| {
        eprintln!("There are no migrations");
        std::process::exit(1);
    });
    let top_migration = ms.latest().expect("Latest should exist");
    if initial_migration == &top_migration {
        eprintln!("Can not detach initial migration");
        std::process::exit(1);
    }
    if let Ok(spec) = db::ConnectionSpec::load(base_dir) {
        let conn = db::connect(&spec).await?;
        if let Some(top_applied_migration) = ms.last_applied_migration(&conn).await? {
            if top_applied_migration == top_migration {
                eprintln!("Can not detach an applied migration");
                std::process::exit(1);
            }
        }
    }
    let previous_migration = &all_migrations[all_migrations.len() - 2];
    println!(
        "Detaching {} from {}",
        top_migration.name(),
        previous_migration.name()
    );
    ms.detach_latest_migration()?;

    // The latest migration needs to be removed from the embedding
    update_embedded(base_dir)?;

    Ok(())
}

pub async fn migrate(base_dir: &PathBuf, name: Option<String>) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let mut conn = db::connect(&spec).await?;
    let to_apply = get_migrations(base_dir)?
        .unapplied_migrations(&conn)
        .await?;
    println!("{} migrations to apply", to_apply.len());
    for m in to_apply {
        println!("Applying migration {}", m.name());
        m.apply(&mut conn).await?;
        if let Some(ref name) = name {
            if name == &m.name().to_string() {
                println!("Finishing at migration {}", m.name());
                break;
            }
        }
    }
    Ok(())
}

pub async fn rollback(base_dir: &PathBuf, name: Option<String>) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let conn = butane::db::connect(&spec).await?;

    match name {
        Some(to) => rollback_to(base_dir, conn, &to).await,
        None => rollback_latest(base_dir, conn).await,
    }
}

pub async fn rollback_to(base_dir: &Path, mut conn: Connection, to: &str) -> Result<()> {
    let ms = get_migrations(base_dir)?;
    let to_migration = match ms.get_migration(to) {
        Some(m) => m,
        None => {
            eprintln!("No such migration!");
            std::process::exit(1);
        }
    };

    let latest = ms
        .last_applied_migration(&conn)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Err: {err}");
            std::process::exit(1);
        })
        .unwrap_or_else(|| {
            eprintln!("No migrations applied!");
            std::process::exit(1);
        });

    if to_migration == latest {
        eprintln!("That is the latest applied migration, not rolling back to anything.");
        std::process::exit(1);
    }

    let mut to_unapply = ms.migrations_since(&to_migration)?;
    if to_unapply.is_empty() {
        return Err(anyhow::anyhow!(
            "That is the latest migration, not rolling back to anything.
If you expected something to happen, try specifying the migration to rollback to."
        ));
    }

    if *to_unapply.last().unwrap() != latest {
        let index = to_unapply
            .iter()
            .position(|m| m.name() == latest.name())
            .unwrap();
        to_unapply = to_unapply.split_at(index + 1).0.into();
    }

    for m in to_unapply.into_iter().rev() {
        println!("Rolling back migration {}", m.name());
        m.downgrade(&mut conn).await?;
    }
    Ok(())
}

pub async fn rollback_latest(base_dir: &Path, mut conn: Connection) -> Result<()> {
    match get_migrations(base_dir)?
        .last_applied_migration(&conn)
        .await?
    {
        Some(m) => {
            println!("Rolling back migration {}", m.name());
            m.downgrade(&mut conn).await?;
        }
        None => {
            eprintln!("No migrations applied!");
            std::process::exit(1)
        }
    };
    Ok(())
}

/// Create `src/butane_migrations.rs` containing the migrations metadata.
pub fn embed(base_dir: &Path) -> Result<()> {
    let srcdir = base_dir.join("../src");
    if !srcdir.is_dir() {
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
    let json = serde_json::to_string_pretty(&mem_ms)?;

    let src = format!(
        "//! Butane migrations embedded in Rust.

use butane::migrations::MemMigrations;

/// Load the butane migrations embedded in Rust.
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {{
    let json = r#\"{json}\"#;
    MemMigrations::from_json(json)
}}
"
    );

    let mut f = std::fs::File::create(path)?;
    f.write_all(src.as_bytes())?;

    let mut cli_state = CliState::load(base_dir)?;
    cli_state.embedded = true;
    cli_state.save(base_dir)?;
    Ok(())
}

/// Update `src/butane_migrations.rs` if embedding is enabled.
pub fn update_embedded(base_dir: &Path) -> Result<()> {
    let cli_state = CliState::load(base_dir)?;
    if cli_state.embedded {
        // Update the embedding
        embed(base_dir)?;
    }
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

/// List backends used in existing migrations.
pub fn list_backends(base_dir: &Path) -> Result<()> {
    let backends = load_latest_migration_backends(base_dir)?;
    for backend in backends {
        println!("{}", backend.name());
    }
    Ok(())
}

/// Add backend to existing migrations.
pub fn add_backend(base_dir: &Path, backend_name: &str) -> Result<()> {
    let existing_backends = load_latest_migration_backends(base_dir)?;

    for backend in existing_backends {
        if backend.name() == backend_name {
            return Err(anyhow::anyhow!(
                "Backend {backend_name} already present in migrations."
            ));
        }
    }

    let backend =
        db::get_backend(backend_name).ok_or(anyhow::anyhow!("Backend {backend_name} not found"))?;

    let migrations = get_migrations(base_dir)?;
    let migration_list = migrations.all_migrations()?;
    let mut from_db = adb::ADB::new();
    for mut m in migration_list {
        println!("Updating {}", m.name());
        let to_db = m.db()?;
        let mut ops = diff(&from_db, &to_db);
        assert!(!ops.is_empty());

        if from_db.tables().count() == 0 {
            // This is the first migration. Create the butane_migration table
            ops.push(adb::Operation::AddTableIfNotExists(
                migrations::migrations_table(),
            ));
        }

        let up_sql = backend.create_migration_sql(&from_db, ops)?;
        let down_sql = backend.create_migration_sql(&to_db, diff(&to_db, &from_db))?;
        m.add_sql(backend.name(), &up_sql, &down_sql)?;

        from_db = to_db;
    }

    update_embedded(base_dir)?;

    Ok(())
}

/// Remove a backend from existing migrations.
pub fn remove_backend(base_dir: &Path, backend_name: &str) -> Result<()> {
    let existing_backends = load_latest_migration_backends(base_dir)?;

    if existing_backends.len() == 1 {
        return Err(anyhow::anyhow!("Can not remove the last backend."));
    }

    let backend =
        db::get_backend(backend_name).ok_or(anyhow::anyhow!("Backend {backend_name} not found"))?;

    let migrations = get_migrations(base_dir)?;
    let migration_list = migrations.all_migrations()?;

    for mut m in migration_list {
        println!("Updating {}", m.name());
        m.remove_sql(backend.name())?;
    }

    update_embedded(base_dir)?;

    Ok(())
}

/// Regenerate migrations.
pub fn regenerate_migrations(base_dir: &Path) -> Result<()> {
    let backends = load_latest_migration_backends(base_dir)?;

    let mut migrations = get_migrations(base_dir)?;
    let migration_list = migrations.all_migrations()?;

    let mut from_migration_name: Option<String> = None;

    for m in migration_list {
        println!("Updating {}", m.name());
        let to_db = m.db()?;
        let mut from_migration = None;
        if let Some(from_migration_name) = from_migration_name {
            from_migration = migrations.get_migration(&from_migration_name);
        }

        m.delete_db()?;
        migrations.create_migration_to(
            &backends,
            &m.name(),
            from_migration.as_ref(),
            to_db.clone(),
        )?;

        from_migration_name = Some(m.name().to_string());
    }

    update_embedded(base_dir)?;

    Ok(())
}

/// Load the [`db::Backend`]s used in the latest migration.
/// Error if there are no existing migrations.
pub fn load_latest_migration_backends(base_dir: &Path) -> Result<NonEmpty<Box<dyn db::Backend>>> {
    if let Ok(ms) = get_migrations(base_dir) {
        if let Some(latest_migration) = ms.latest() {
            let backend_names = latest_migration.sql_backends()?;
            assert!(!backend_names.is_empty());
            log::info!(
                "Latest migration contains backends: {}",
                backend_names.join(", ")
            );

            let mut backends: Vec<Box<dyn db::Backend>> = vec![];

            for backend_name in backend_names {
                backends.push(
                    db::get_backend(&backend_name)
                        .ok_or(anyhow::anyhow!("Backend {backend_name} not found"))?,
                );
            }

            return Ok(NonEmpty::<Box<dyn db::Backend>>::from_vec(backends).unwrap());
        }
    }
    Err(anyhow::anyhow!("There are no exiting migrations."))
}

/// Load [`db::Backend`]s selected in the latest migration, or when there are no migrations,
/// fallback to the backend named in the connection.
pub fn load_backends(base_dir: &Path) -> Result<NonEmpty<Box<dyn db::Backend>>> {
    // Try to use the same backends as the latest migration.
    let backends = load_latest_migration_backends(base_dir);
    if backends.is_ok() {
        return backends;
    }

    // Otherwise use the backend used during `init`.
    if let Ok(spec) = db::ConnectionSpec::load(base_dir) {
        return Ok(nonempty::nonempty![spec.get_backend().unwrap()]);
    }

    Err(anyhow::anyhow!(
        "No Butane connection info found. Run `butane init`"
    ))
}

pub async fn list_migrations(base_dir: &PathBuf) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let conn = db::connect(&spec).await?;
    let ms = get_migrations(base_dir)?;
    let unapplied = ms.unapplied_migrations(&conn).await?;
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

/// Collapse multiple applied migrations into a new migration.
pub async fn collapse_migrations(
    base_dir: &PathBuf,
    new_initial_name: Option<&String>,
) -> Result<()> {
    let name = match new_initial_name {
        Some(name) => format!("{}_{}", default_name(), name),
        None => default_name(),
    };

    let mut ms = get_migrations(base_dir)?;

    let all_migrations = ms.all_migrations().unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });
    let initial_migration = all_migrations.first().unwrap_or_else(|| {
        eprintln!("There are no migrations to collapse");
        std::process::exit(1);
    });
    let latest_migration = ms.latest().expect("Latest should exist");
    if initial_migration == &latest_migration {
        eprintln!("Can not collapse a single migration");
        std::process::exit(1);
    }

    // Use the same backends as the latest migration.
    let backends = load_latest_migration_backends(base_dir)?;

    // TODO: it should also be possible to collapse migrations on the filesystem
    // when the database hasnt been migrated at all.
    let spec = load_connspec(base_dir)?;
    let conn = db::connect(&spec).await?;
    let latest = ms.last_applied_migration(&conn).await?;
    if latest.is_none() {
        eprintln!("There are no applied migrations to collapse");
        std::process::exit(1);
    }

    let latest_db = latest.unwrap().db()?;
    ms.clear_migrations(&conn).await?;
    ms.create_migration_to(&backends, &name, None, latest_db)?;
    let new_migration = ms.latest().unwrap();
    new_migration.mark_applied(&conn).await?;

    update_embedded(base_dir)?;

    println!("Collapsed all changes into new single migration '{name}'");
    Ok(())
}

pub fn delete_table(base_dir: &Path, name: &str) -> Result<()> {
    let mut ms = get_migrations(base_dir)?;
    let current = ms.current();
    current.delete_table(name)?;
    Ok(())
}

pub async fn clear_data(base_dir: &PathBuf) -> Result<()> {
    let spec = load_connspec(base_dir)?;
    let conn = db::connect(&spec).await?;
    let latest = match get_migrations(base_dir)?
        .last_applied_migration(&conn)
        .await?
    {
        Some(m) => m,
        None => {
            eprintln!("No migrations have been applied, so no data is recognized.");
            std::process::exit(1);
        }
    };
    for table in latest.db()?.tables() {
        println!("Deleting data from {}", &table.name);
        conn.delete_where(&table.name, BoolExpr::True).await?;
    }
    Ok(())
}

pub fn clean(base_dir: &Path) -> Result<()> {
    get_migrations(base_dir)?.clear_current()?;
    Ok(())
}

pub fn get_migrations(base_dir: &Path) -> Result<FsMigrations> {
    let root = base_dir.join("migrations");
    if !root.is_dir() {
        eprintln!("No butane migrations directory found. Add at least one model to your project and build.");
        std::process::exit(1);
    }
    Ok(migrations::from_root(root))
}

pub fn working_dir_path() -> PathBuf {
    match std::env::current_dir() {
        Ok(path) => path,
        Err(_) => PathBuf::from("."),
    }
}

/// Extract the directory of a cargo workspace member identified by PackageId
pub fn extract_package_directory(
    packages: &[cargo_metadata::Package],
    package_id: cargo_metadata::PackageId,
) -> Result<std::path::PathBuf> {
    let pkg = packages
        .iter()
        .find(|p| p.id == package_id)
        .ok_or(anyhow::anyhow!("No package found"))?;
    // Strip 'Cargo.toml' from the manifest_path
    let parent = pkg.manifest_path.parent().unwrap();
    Ok(parent.to_owned().into())
}

/// Find all cargo workspace members that have a `.butane` subdirectory
pub fn find_butane_workspace_member_paths() -> Result<Vec<PathBuf>> {
    let metadata = MetadataCommand::new().no_deps().exec()?;
    let workspace_members = metadata.workspace_members;

    let mut possible_directories: Vec<PathBuf> = vec![];
    // Find all workspace member with a .butane
    for member in workspace_members {
        let package_dir = extract_package_directory(&metadata.packages, member)?;
        let member_butane_dir = package_dir.join(".butane/");

        if member_butane_dir.is_dir() {
            possible_directories.push(package_dir);
        }
    }
    Ok(possible_directories)
}

/// Get the project path if only one workspace member contains a `.butane` directory
pub fn get_butane_project_path() -> Result<PathBuf> {
    let possible_directories = find_butane_workspace_member_paths()?;

    match possible_directories.len() {
        0 => Err(anyhow::anyhow!("No .butane exists")),
        1 => Ok(possible_directories[0].to_owned()),
        _ => Err(anyhow::anyhow!("Multiple .butane exists")),
    }
}

/// Find a .butane directory to act as the base for butane.
pub fn base_dir() -> PathBuf {
    let current_directory = working_dir_path();
    let local_butane_dir = current_directory.join(".butane/");

    if !local_butane_dir.is_dir() {
        if let Ok(member_dir) = get_butane_project_path() {
            println!("Using workspace member {:?}", member_dir);
            return member_dir;
        }
    }

    // Fallback to the current directory
    current_directory
}

pub fn handle_error(r: Result<()>) {
    if let Err(e) = r {
        eprintln!("Encountered unexpected error: {e}");
        std::process::exit(1);
    }
}
