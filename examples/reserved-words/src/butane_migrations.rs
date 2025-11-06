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
        "libsql": "CREATE TABLE \"User\" (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n) STRICT;\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\n",
        "pg": "CREATE TABLE \"User\" (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE \"User\" (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n) STRICT;\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\n",
        "turso": "CREATE TABLE \"User\" (\n\"id\" TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\nemail TEXT NOT NULL\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n"
      },
      "down": {
        "libsql": "DROP TABLE \"User\";\n",
        "pg": "DROP TABLE \"User\";\n",
        "sqlite": "DROP TABLE \"User\";\n",
        "turso": "DROP TABLE \"User\";\n"
      }
    },
    "20250504_001654915_add-fkey": {
      "name": "20250504_001654915_add-fkey",
      "db": {
        "tables": {
          "Post": {
            "name": "Post",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": true,
                "unique": false,
                "default": null
              },
              {
                "name": "title",
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
                "name": "body",
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
                "name": "published",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Bool"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "byline",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": true,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              }
            ]
          },
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
      "from": "20250503_065948927_init",
      "up": {
        "libsql": "CREATE TABLE Post (\n\"id\" INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nbyline TEXT,\nFOREIGN KEY (byline) REFERENCES \"User\"(\"id\")\n) STRICT;\n",
        "pg": "CREATE TABLE Post (\n\"id\" SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nbyline TEXT\n);\nALTER TABLE Post ADD FOREIGN KEY (byline) REFERENCES \"User\"(\"id\");\n",
        "sqlite": "CREATE TABLE Post (\n\"id\" INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nbyline TEXT,\nFOREIGN KEY (byline) REFERENCES \"User\"(\"id\")\n) STRICT;\n",
        "turso": "CREATE TABLE Post (\n\"id\" INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nbyline TEXT,\nFOREIGN KEY (byline) REFERENCES \"User\"(\"id\")\n);\n"
      },
      "down": {
        "libsql": "DROP TABLE Post;\n",
        "pg": "ALTER TABLE Post DROP CONSTRAINT Post_byline_fkey;\nDROP TABLE Post;\n",
        "sqlite": "DROP TABLE Post;\n",
        "turso": "DROP TABLE Post;\n"
      }
    },
    "20250504_002109086_add-many": {
      "name": "20250504_002109086_add-many",
      "db": {
        "tables": {
          "Post": {
            "name": "Post",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": true,
                "unique": false,
                "default": null
              },
              {
                "name": "title",
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
                "name": "body",
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
                "name": "published",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Bool"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "byline",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": true,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              }
            ]
          },
          "Post_likes_Many": {
            "name": "Post_likes_Many",
            "columns": [
              {
                "name": "owner",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "Post",
                    "column_name": "id"
                  }
                }
              },
              {
                "name": "has",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              }
            ]
          },
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
      "from": "20250504_001654915_add-fkey",
      "up": {
        "libsql": "CREATE TABLE Post_likes_Many (\n\"owner\" INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (\"owner\") REFERENCES Post(\"id\")\nFOREIGN KEY (has) REFERENCES \"User\"(\"id\")\n) STRICT;\n",
        "pg": "CREATE TABLE Post_likes_Many (\n\"owner\" INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nALTER TABLE Post_likes_Many ADD FOREIGN KEY (\"owner\") REFERENCES Post(\"id\");\nALTER TABLE Post_likes_Many ADD FOREIGN KEY (has) REFERENCES \"User\"(\"id\");\n",
        "sqlite": "CREATE TABLE Post_likes_Many (\n\"owner\" INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (\"owner\") REFERENCES Post(\"id\")\nFOREIGN KEY (has) REFERENCES \"User\"(\"id\")\n) STRICT;\n",
        "turso": "CREATE TABLE Post_likes_Many (\n\"owner\" INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (\"owner\") REFERENCES Post(\"id\")\nFOREIGN KEY (has) REFERENCES \"User\"(\"id\")\n);\n"
      },
      "down": {
        "libsql": "DROP TABLE Post_likes_Many;\n",
        "pg": "ALTER TABLE Post_likes_Many DROP CONSTRAINT Post_likes_Many_owner_fkey;\nALTER TABLE Post_likes_Many DROP CONSTRAINT Post_likes_Many_has_fkey;\nDROP TABLE Post_likes_Many;\n",
        "sqlite": "DROP TABLE Post_likes_Many;\n",
        "turso": "DROP TABLE Post_likes_Many;\n"
      }
    },
    "20250504_025454048_rowid": {
      "name": "20250504_025454048_rowid",
      "db": {
        "tables": {
          "Post": {
            "name": "Post",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": true,
                "unique": false,
                "default": null
              },
              {
                "name": "title",
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
                "name": "body",
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
                "name": "published",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Bool"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "byline",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": true,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              }
            ]
          },
          "Post_likes_Many": {
            "name": "Post_likes_Many",
            "columns": [
              {
                "name": "owner",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "Post",
                    "column_name": "id"
                  }
                }
              },
              {
                "name": "has",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Text"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              }
            ]
          },
          "RowidTest": {
            "name": "RowidTest",
            "columns": [
              {
                "name": "rowid",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": false,
                "unique": false,
                "default": null
              }
            ]
          },
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
      "from": "20250504_002109086_add-many",
      "up": {
        "libsql": "CREATE TABLE RowidTest (\n\"rowid\" INTEGER NOT NULL PRIMARY KEY\n) STRICT;\n",
        "pg": "CREATE TABLE RowidTest (\n\"rowid\" INTEGER NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE RowidTest (\n\"rowid\" INTEGER NOT NULL PRIMARY KEY\n) STRICT;\n",
        "turso": "CREATE TABLE RowidTest (\n\"rowid\" INTEGER NOT NULL PRIMARY KEY\n);\n"
      },
      "down": {
        "libsql": "DROP TABLE RowidTest;\n",
        "pg": "DROP TABLE RowidTest;\n",
        "sqlite": "DROP TABLE RowidTest;\n",
        "turso": "DROP TABLE RowidTest;\n"
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
  "latest": "20250504_025454048_rowid"
}"#;
    MemMigrations::from_json(json)
}
