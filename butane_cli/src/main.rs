use std::path::PathBuf;

use butane_cli::{
    base_dir, clean, clear_data, collapse_migrations, delete_table, detach_latest_migration, embed,
    get_migrations, handle_error, list_migrations, migrate, Result,
};
use clap::{value_parser, Arg, ArgMatches, ArgAction};

fn main() {
    let app = clap::Command::new("butane")
        .version(env!("CARGO_PKG_VERSION"))
        .author("James Oakley <james@electronstudio.org>")
        .about("Manages butane database migrations")
        .subcommand_required(true)
        .max_term_width(80)
        .arg(
            Arg::new("path").short('p').long("path")
            .default_value(base_dir().into_os_string())
            .value_parser(value_parser!(PathBuf))
            .help("Select directory to locate butane state")
        )
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
                )
                .arg(
                    Arg::new("connect")
                    .long("no-connect")
                    .action(ArgAction::SetFalse)
                    .required(false)
                    .num_args(0)
                    .help("Do not connect to the database"),
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
        .subcommand(
            clap::Command::new("detach-migration")
                .about("Detach the latest migration")
                .alias("detachmigration")
                .after_help(r#"This command removes the latest migration from the list of migrations and sets butane state to before the latest migration was created.

The removed migration is not deleted from file system.

This operation is the first step of the process of rebasing a migration onto other migrations that have the same original migration.

If the migration has not been manually edited, it can be automatically regenerated after being rebased. In this case, deleting the detached migration is often the best approach.

However if the migration has been manually edited, it will need to be manually re-attached to the target migration series after the rebase has been completed.
"#
                )
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
    let mut base_dir = args.get_one::<PathBuf>("path").unwrap().clone();
    base_dir.push(".butane");

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

    match args.subcommand() {
        Some(("init", sub_args)) => handle_error(init(&base_dir, Some(sub_args))),
        Some(("makemigration", sub_args)) => {
            handle_error(make_migration(&base_dir, Some(sub_args)))
        }
        Some(("detach-migration", _)) => handle_error(detach_latest_migration(&base_dir)),
        Some(("migrate", _)) => handle_error(migrate(&base_dir)),
        Some(("rollback", sub_args)) => handle_error(rollback(&base_dir, Some(sub_args))),
        Some(("embed", _)) => handle_error(embed(&base_dir)),
        Some(("list", _)) => handle_error(list_migrations(&base_dir)),
        Some(("collapse", sub_args)) => {
            handle_error(collapse_migrations(&base_dir, sub_args.get_one("NAME")))
        }
        Some(("clear", sub_args)) => match sub_args.subcommand() {
            Some(("data", _)) => handle_error(clear_data(&base_dir)),
            _ => eprintln!("Unknown clear command. Try: clear data"),
        },
        Some(("delete", sub_args)) => match sub_args.subcommand() {
            Some(("table", sub_args2)) => handle_error(delete_table(
                &base_dir,
                sub_args2.get_one::<&str>("TABLE").unwrap(),
            )),
            _ => eprintln!("Unknown delete command. Try: delete table"),
        },
        Some(("clean", _)) => handle_error(clean(&base_dir)),
        Some((_, _)) | None => panic!("Unreachable as clap handles this automatically"),
    }
}

fn init(base_dir: &PathBuf, args: Option<&ArgMatches>) -> Result<()> {
    let args = args.unwrap();
    let name: &String = args.get_one("BACKEND").unwrap();
    let connstr: &String = args.get_one("CONNECTION").unwrap();
    let connect: bool = *args.get_one::<bool>("connect").unwrap();
    butane_cli::init(base_dir, name, connstr, connect)
}

fn make_migration(base_dir: &PathBuf, args: Option<&ArgMatches>) -> Result<()> {
    let name_arg = args.and_then(|a| a.get_one::<String>("NAME"));
    butane_cli::make_migration(base_dir, name_arg)
}

fn rollback(base_dir: &PathBuf, args: Option<&ArgMatches>) -> Result<()> {
    let spec = butane_cli::load_connspec(base_dir)?;
    let conn = butane::db::connect(&spec)?;

    match args.and_then(|a| a.get_one::<String>("NAME")) {
        Some(to) => butane_cli::rollback_to(base_dir, conn, to),
        None => butane_cli::rollback_latest(base_dir, conn),
    }
}
