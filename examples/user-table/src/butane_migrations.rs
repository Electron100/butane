//! Butane migrations embedded in Rust.

use butane::migrations::MemMigrations;

/// Load the butane migrations embedded in Rust.
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {
    let json = r#"{
  "migrations": {
    "20250503_065948927_init": {
      "name": "20250503_065948927_init",
      "db": {
        "tables": {
          "User": {
            "name": "User",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "name",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "email",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              }
            ]
          }
        },
        "extra_types": {}
      },
      "from": null,
      "up": {
        "pg": "CREATE TABLE \"User\" (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE User (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n) STRICT;\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\n"
      },
      "down": {
        "pg": "DROP TABLE \"User\";\n",
        "sqlite": "DROP TABLE \"User\";\n"
      }
    }
  },
  "current": {
    "name": "current",
    "db": {
      "tables": {},
      "extra_types": {}
    },
    "from": null,
    "up": {},
    "down": {}
  },
  "latest": "20250503_065948927_init"
}"#;
    MemMigrations::from_json(json)
}
