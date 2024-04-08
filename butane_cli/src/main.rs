//! butane CLI.
#![deny(missing_docs)]

use std::path::PathBuf;

use butane_cli::{
    add_backend, base_dir, clean, clear_data, collapse_migrations, delete_table,
    describe_migration, detach_latest_migration, embed, get_migrations, handle_error, init,
    list_backends, list_migrations, make_migration, migrate, regenerate_migrations, remove_backend,
    rollback,
};
use clap::{ArgAction, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "Manages butane database migrations.")]
#[command(propagate_version = true, max_term_width = 80)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short = 'p', long, default_value=base_dir().into_os_string())]
    path: PathBuf,
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the database.
    Init(InitCommand),
    /// Backends.
    Backend {
        #[clap(subcommand)]
        subcommand: BackendCommands,
    },
    /// Create a new migration.
    #[command(alias = "makemigration")]
    MakeMigration {
        /// Name to use for the migration.
        name: String,
    },
    /// Detach the latest migration.
    #[command(
        alias = "detachmigration",
        after_help = "This command removes the latest migration from the list of migrations and sets butane state to before the latest migration was created.

The removed migration is not deleted from file system.

This operation is the first step of the process of rebasing a migration onto other migrations that have the same original migration.

If the migration has not been manually edited, it can be automatically regenerated after being rebased. In this case, deleting the detached migration is often the best approach.

However if the migration has been manually edited, it will need to be manually re-attached to the target migration series after the rebase has been completed.
"
    )]
    DetachMigration,
    /// Apply migrations.
    Migrate {
        /// Migration to migrate to.
        name: Option<String>,
    },
    /// Regenerate migrations in place.
    Regenerate,
    DescribeMigration {
        /// Name of migration to be described, or `current`.
        name: String,
    },
    /// List migrations.
    List,
    /// Replace all migrations with a single migration representing the current model state.
    Collapse {
        /// Name to use for the new migration.
        name: String,
    },
    /// Embed migrations in the source code.
    Embed,
    /// Rollback migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration.
    Rollback {
        /// Migration to roll back to.
        name: Option<String>,
    },
    /// Clear.
    Clear {
        #[clap(subcommand)]
        subcommand: ClearCommands,
    },
    /// Delete.
    Delete {
        #[clap(subcommand)]
        subcommand: DeleteCommands,
    },
    /// Clean current migration state. Deletes the current migration working state which is generated on each build. This can be used as a workaround to remove stale tables from the schema, as Butane does not currently auto-detect model removals. The next build will recreate with only tables for the extant models.
    Clean,
}

#[derive(Parser)]
struct InitCommand {
    /// Database connection string. Format depends on backend.
    backend: String,
    /// Database backend to use. 'sqlite' or 'pg'.
    connection: String,
    /// Do not connect to the database.
    #[arg(required = false, long="no-connect", action = ArgAction::SetFalse)]
    connect: bool,
}

#[derive(Subcommand)]
enum BackendCommands {
    /// Add a backend to existing migrations.
    Add {
        /// Backend name to add.
        name: String,
    },
    /// Remove a backend from existing migrations.
    Remove {
        /// Backend name to remove.
        name: String,
    },
    /// List backends present in existing migrations.
    List,
}

#[derive(Subcommand)]
enum ClearCommands {
    /// Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted.
    Data,
}

#[derive(Subcommand)]
enum DeleteCommands {
    /// Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted.
    Table {
        /// Table name.
        name: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let mut base_dir = cli.path;
    if !base_dir.ends_with(".butane") {
        base_dir.push(".butane");
    }

    // List any detached migrations.
    if let Ok(ms) = get_migrations(&base_dir) {
        if let Ok(detached_migrations) = ms.detached_migration_paths() {
            if !detached_migrations.is_empty() {
                eprintln!(
                    "Ignoring detached migrations. Please delete or manually re-attach these:"
                );
                for migration in detached_migrations {
                    eprintln!("- {migration}");
                }
            }
        };
    };

    match &cli.command {
        Commands::Init(args) => {
            handle_error(init(&base_dir, &args.backend, &args.connection, args.connect).await)
        }
        Commands::Backend { subcommand } => match subcommand {
            BackendCommands::Add { name } => handle_error(add_backend(&base_dir, name)),
            BackendCommands::Remove { name } => handle_error(remove_backend(&base_dir, name)),
            BackendCommands::List => handle_error(list_backends(&base_dir)),
        },
        Commands::MakeMigration { name } => handle_error(make_migration(&base_dir, Some(name))),
        Commands::DescribeMigration { name } => handle_error(describe_migration(&base_dir, name)),
        Commands::Regenerate => handle_error(regenerate_migrations(&base_dir)),
        Commands::DetachMigration => handle_error(detach_latest_migration(&base_dir).await),
        Commands::Migrate { name } => handle_error(migrate(&base_dir, name.to_owned()).await),
        Commands::Rollback { name } => handle_error(rollback(&base_dir, name.to_owned()).await),
        Commands::Embed => handle_error(embed(&base_dir)),
        Commands::List => handle_error(list_migrations(&base_dir).await),
        Commands::Collapse { name } => {
            handle_error(collapse_migrations(&base_dir, Some(name)).await)
        }
        Commands::Clear { subcommand } => match subcommand {
            ClearCommands::Data => handle_error(clear_data(&base_dir).await),
        },
        Commands::Delete { subcommand } => match subcommand {
            DeleteCommands::Table { name } => handle_error(delete_table(&base_dir, name)),
        },
        Commands::Clean => handle_error(clean(&base_dir)),
    }
}
