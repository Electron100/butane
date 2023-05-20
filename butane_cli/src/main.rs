use clap::{Arg, ArgMatches};

use butane_cli::{
    clean, clear_data, collapse_migrations, delete_table, embed, handle_error, list_migrations,
    migrate, Result,
};

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

fn init(args: Option<&ArgMatches>) -> Result<()> {
    let args = args.unwrap();
    let name: &String = args.get_one("BACKEND").unwrap();
    let connstr: &String = args.get_one("CONNECTION").unwrap();
    butane_cli::init(name, connstr)
}

fn make_migration(args: Option<&ArgMatches>) -> Result<()> {
    let name_arg = args.and_then(|a| a.get_one::<String>("NAME"));
    butane_cli::make_migration(name_arg)
}

fn rollback(args: Option<&ArgMatches>) -> Result<()> {
    let spec = butane_cli::load_connspec()?;
    let conn = butane::db::connect(&spec)?;

    match args.and_then(|a| a.get_one::<String>("NAME")) {
        Some(to) => butane_cli::rollback_to(conn, to),
        None => butane_cli::rollback_latest(conn),
    }
}
