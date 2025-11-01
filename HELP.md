# Command-Line Help for `butane`

This document contains the help content for the `butane` command-line program.

**Command Overview:**

* [`butane`↴](#butane)
* [`butane init`↴](#butane-init)
* [`butane backend`↴](#butane-backend)
* [`butane backend add`↴](#butane-backend-add)
* [`butane backend remove`↴](#butane-backend-remove)
* [`butane backend list`↴](#butane-backend-list)
* [`butane make-migration`↴](#butane-make-migration)
* [`butane detach-migration`↴](#butane-detach-migration)
* [`butane migrate`↴](#butane-migrate)
* [`butane regenerate`↴](#butane-regenerate)
* [`butane describe-migration`↴](#butane-describe-migration)
* [`butane list`↴](#butane-list)
* [`butane collapse`↴](#butane-collapse)
* [`butane embed`↴](#butane-embed)
* [`butane unmigrate`↴](#butane-unmigrate)
* [`butane clear`↴](#butane-clear)
* [`butane clear data`↴](#butane-clear-data)
* [`butane delete`↴](#butane-delete)
* [`butane delete table`↴](#butane-delete-table)
* [`butane clean`↴](#butane-clean)

## `butane`

Manages butane database migrations.

**Usage:** `butane [OPTIONS] <COMMAND>`

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

* `-v`, `--verbose` — Increase logging verbosity
* `-q`, `--quiet` — Decrease logging verbosity
* `-p`, `--path <PATH>`

  Default value: `<detected project containing .butane directory>`



## `butane init`

Initialize the database

**Usage:** `butane init [OPTIONS] <BACKEND> <CONNECTION>`

###### **Arguments:**

* `<BACKEND>` — Database connection string. Format depends on backend
* `<CONNECTION>` — Database backend to use. 'sqlite' or 'pg'

###### **Options:**

* `--no-connect` — Do not connect to the database

  Possible values: `true`, `false`




## `butane backend`

Backends

**Usage:** `butane backend <COMMAND>`

###### **Subcommands:**

* `add` — Add a backend to existing migrations
* `remove` — Remove a backend from existing migrations
* `list` — List backends present in existing migrations



## `butane backend add`

Add a backend to existing migrations

**Usage:** `butane backend add <NAME>`

###### **Arguments:**

* `<NAME>` — Backend name to add



## `butane backend remove`

Remove a backend from existing migrations

**Usage:** `butane backend remove <NAME>`

###### **Arguments:**

* `<NAME>` — Backend name to remove



## `butane backend list`

List backends present in existing migrations

**Usage:** `butane backend list`



## `butane make-migration`

Create a new migration

**Usage:** `butane make-migration <NAME>`

###### **Arguments:**

* `<NAME>` — Name to use for the migration



## `butane detach-migration`

Detach the latest migration

**Usage:** `butane detach-migration`

This command removes the latest migration from the list of migrations and sets butane state to before the latest migration was created.

The removed migration is not deleted from file system.

This operation is the first step of the process of rebasing a migration onto other migrations that have the same original migration.

If the migration has not been manually edited, it can be automatically regenerated after being rebased. In this case, deleting the detached migration is often the best approach.

However if the migration has been manually edited, it will need to be manually re-attached to the target migration series after the rebase has been completed.




## `butane migrate`

Apply migrations

**Usage:** `butane migrate [NAME]`

###### **Arguments:**

* `<NAME>` — Migration to migrate to



## `butane regenerate`

Regenerate migrations in place

**Usage:** `butane regenerate`



## `butane describe-migration`

**Usage:** `butane describe-migration <NAME>`

###### **Arguments:**

* `<NAME>` — Name of migration to be described, or `current`



## `butane list`

List migrations

**Usage:** `butane list`



## `butane collapse`

Replace all migrations with a single migration representing the current model state

**Usage:** `butane collapse <NAME>`

###### **Arguments:**

* `<NAME>` — Name to use for the new migration



## `butane embed`

Embed migrations in the source code

**Usage:** `butane embed`



## `butane unmigrate`

Undo migrations. With no arguments, undoes the latest migration. If the name of a migration is specified, rolls back until that migration is the latest applied migration

**Usage:** `butane unmigrate [NAME]`

###### **Arguments:**

* `<NAME>` — Migration to roll back to



## `butane clear`

Clear

**Usage:** `butane clear <COMMAND>`

###### **Subcommands:**

* `data` — Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted



## `butane clear data`

Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted

**Usage:** `butane clear data`



## `butane delete`

Delete

**Usage:** `butane delete <COMMAND>`

###### **Subcommands:**

* `table` — Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted



## `butane delete table`

Clear all data from the database. The schema is left intact, but all instances of all models (i.e. all rows of all tables defined by the models) are deleted

**Usage:** `butane delete table <NAME>`

###### **Arguments:**

* `<NAME>` — Table name



## `butane clean`

Clean current migration state. Deletes the current migration working state which is generated on each build. This can be used as a workaround to remove stale tables from the schema, as Butane does not currently auto-detect model removals. The next build will recreate with only tables for the extant models

**Usage:** `butane clean`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

