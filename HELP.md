# Command-Line Help for `butane_cli`

This document contains the help content for the `butane_cli` command-line program.

**Command Overview:**

* [`butane_cli`↴](#butane_cli)
* [`butane_cli init`↴](#butane_cli-init)
* [`butane_cli backend`↴](#butane_cli-backend)
* [`butane_cli backend add`↴](#butane_cli-backend-add)
* [`butane_cli backend remove`↴](#butane_cli-backend-remove)
* [`butane_cli backend list`↴](#butane_cli-backend-list)
* [`butane_cli make-migration`↴](#butane_cli-make-migration)
* [`butane_cli detach-migration`↴](#butane_cli-detach-migration)
* [`butane_cli migrate`↴](#butane_cli-migrate)
* [`butane_cli regenerate`↴](#butane_cli-regenerate)
* [`butane_cli describe-migration`↴](#butane_cli-describe-migration)
* [`butane_cli list`↴](#butane_cli-list)
* [`butane_cli collapse`↴](#butane_cli-collapse)
* [`butane_cli embed`↴](#butane_cli-embed)
* [`butane_cli unmigrate`↴](#butane_cli-unmigrate)
* [`butane_cli clear`↴](#butane_cli-clear)
* [`butane_cli clear data`↴](#butane_cli-clear-data)
* [`butane_cli delete`↴](#butane_cli-delete)
* [`butane_cli delete table`↴](#butane_cli-delete-table)
* [`butane_cli clean`↴](#butane_cli-clean)

## `butane_cli`

Manages butane database migrations.

**Usage:** `butane_cli [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `init` — Initialize the database
* `backend` — Backends
* `make-migration` — Create a new migration
* `detach-migration` — Detach the latest migration
* `migrate` — Apply migrations
* `regenerate` — Regenerate migrations in place
* `describe-migration` — 
* `list` — List migrations
* `collapse` — Replace all migrations with a single migration representing the current model state
* `embed` — Embed migrations in the source code
* `unmigrate` — Undo migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration
* `clear` — Clear
* `delete` — Delete
* `clean` — Clean current migration state. Deletes the current migration working state which is generated on each build. This can be used as a workaround to remove stale tables from the schema, as Butane does not currently auto-detect model removals. The next build will recreate with only tables for the extant models

###### **Options:**

* `-p`, `--path <PATH>`

  Default value: `<detected project containing .butane directory>`
* `-v`, `--verbose` — Increase logging verbosity
* `-q`, `--quiet` — Decrease logging verbosity



## `butane_cli init`

Initialize the database

**Usage:** `butane_cli init [OPTIONS] <BACKEND> <CONNECTION>`

###### **Arguments:**

* `<BACKEND>` — Database connection string. Format depends on backend
* `<CONNECTION>` — Database backend to use. 'sqlite' or 'pg'

###### **Options:**

* `--no-connect` — Do not connect to the database

  Possible values: `true`, `false`




## `butane_cli backend`

Backends

**Usage:** `butane_cli backend <COMMAND>`

###### **Subcommands:**

* `add` — Add a backend to existing migrations
* `remove` — Remove a backend from existing migrations
* `list` — List backends present in existing migrations



## `butane_cli backend add`

Add a backend to existing migrations

**Usage:** `butane_cli backend add <NAME>`

###### **Arguments:**

* `<NAME>` — Backend name to add



## `butane_cli backend remove`

Remove a backend from existing migrations

**Usage:** `butane_cli backend remove <NAME>`

###### **Arguments:**

* `<NAME>` — Backend name to remove



## `butane_cli backend list`

List backends present in existing migrations

**Usage:** `butane_cli backend list`



## `butane_cli make-migration`

Create a new migration

**Usage:** `butane_cli make-migration <NAME>`

###### **Arguments:**

* `<NAME>` — Name to use for the migration



## `butane_cli detach-migration`

Detach the latest migration

**Usage:** `butane_cli detach-migration`

This command removes the latest migration from the list of migrations and sets butane state to before the latest migration was created.

The removed migration is not deleted from file system.

This operation is the first step of the process of rebasing a migration onto other migrations that have the same original migration.

If the migration has not been manually edited, it can be automatically regenerated after being rebased. In this case, deleting the detached migration is often the best approach.

However if the migration has been manually edited, it will need to be manually re-attached to the target migration series after the rebase has been completed.




## `butane_cli migrate`

Apply migrations

**Usage:** `butane_cli migrate [NAME]`

###### **Arguments:**

* `<NAME>` — Migration to migrate to



## `butane_cli regenerate`

Regenerate migrations in place

**Usage:** `butane_cli regenerate`



## `butane_cli describe-migration`

**Usage:** `butane_cli describe-migration <NAME>`

###### **Arguments:**

* `<NAME>` — Name of migration to be described, or `current`



## `butane_cli list`

List migrations

**Usage:** `butane_cli list`



## `butane_cli collapse`

Replace all migrations with a single migration representing the current model state

**Usage:** `butane_cli collapse <NAME>`

###### **Arguments:**

* `<NAME>` — Name to use for the new migration



## `butane_cli embed`

Embed migrations in the source code

**Usage:** `butane_cli embed`



## `butane_cli unmigrate`

Undo migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration

**Usage:** `butane_cli unmigrate [NAME]`

###### **Arguments:**

* `<NAME>` — Migration to roll back to



## `butane_cli clear`

Clear

**Usage:** `butane_cli clear <COMMAND>`

###### **Subcommands:**

* `data` — Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted



## `butane_cli clear data`

Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted

**Usage:** `butane_cli clear data`



## `butane_cli delete`

Delete

**Usage:** `butane_cli delete <COMMAND>`

###### **Subcommands:**

* `table` — Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted



## `butane_cli delete table`

Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted

**Usage:** `butane_cli delete table <NAME>`

###### **Arguments:**

* `<NAME>` — Table name



## `butane_cli clean`

Clean current migration state. Deletes the current migration working state which is generated on each build. This can be used as a workaround to remove stale tables from the schema, as Butane does not currently auto-detect model removals. The next build will recreate with only tables for the extant models

**Usage:** `butane_cli clean`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

