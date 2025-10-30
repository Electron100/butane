//! Butane migrations embedded in Rust.

use butane::migrations::MemMigrations;

/// Load the butane migrations embedded in Rust.
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {
    let json = r#"{
  "migrations": {
    "20240401_095709389_init": {
      "name": "20240401_095709389_init",
      "db": {
        "tables": {
          "Blog": {
            "name": "Blog",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
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
              }
            ]
          },
          "Post": {
            "name": "Post",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": false,
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
                "name": "blog",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "Blog",
                    "column_name": "id"
                  }
                }
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
                "default": null
              },
              {
                "name": "likes",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              }
            ]
          },
          "Post_tags_Many": {
            "name": "Post_tags_Many",
            "columns": [
              {
                "name": "owner",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
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
                    "table_name": "Tag",
                    "column_name": "tag"
                  }
                }
              }
            ]
          },
          "Tag": {
            "name": "Tag",
            "columns": [
              {
                "name": "tag",
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
              }
            ]
          }
        },
        "extra_types": {}
      },
      "from": null,
      "up": {
        "mysql": "CREATE TABLE Blog (\n`id` VARBINARY(255) NOT NULL PRIMARY KEY,\n`name` TEXT NOT NULL\n);\nCREATE TABLE Post (\n`id` VARBINARY(255) NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog VARBINARY(255) NOT NULL,\nbyline TEXT,\nlikes INT NOT NULL\n);\nCREATE TABLE Post_tags_Many (\n`owner` VARBINARY(255) NOT NULL,\nhas VARCHAR(255) NOT NULL\n);\nCREATE TABLE `Tag` (\n`tag` VARCHAR(255) NOT NULL PRIMARY KEY\n);\nALTER TABLE Post ADD CONSTRAINT Post_blog_fkey FOREIGN KEY (blog) REFERENCES Blog(`id`);\nALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_owner_fkey FOREIGN KEY (`owner`) REFERENCES Post(`id`);\nALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_has_fkey FOREIGN KEY (has) REFERENCES `Tag`(`tag`);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n`name` VARCHAR(255) NOT NULL PRIMARY KEY\n);\n",
        "pg": "CREATE TABLE Blog (\n\"id\" BYTEA NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nCREATE TABLE Post (\n\"id\" BYTEA NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BYTEA NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nCREATE TABLE Post_tags_Many (\n\"owner\" BYTEA NOT NULL,\nhas TEXT NOT NULL\n);\nCREATE TABLE \"Tag\" (\n\"tag\" TEXT NOT NULL PRIMARY KEY\n);\nALTER TABLE Post ADD FOREIGN KEY (blog) REFERENCES Blog(\"id\");\nALTER TABLE Post_tags_Many ADD FOREIGN KEY (\"owner\") REFERENCES Post(\"id\");\nALTER TABLE Post_tags_Many ADD FOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\");\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE Blog (\n\"id\" BLOB NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n) STRICT;\nCREATE TABLE Post (\n\"id\" BLOB NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog BLOB NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(\"id\")\n) STRICT;\nCREATE TABLE Post_tags_Many (\n\"owner\" BLOB NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (\"owner\") REFERENCES Post(\"id\")\nFOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\")\n) STRICT;\nCREATE TABLE \"Tag\" (\n\"tag\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\n",
        "turso": "CREATE TABLE Blog (\"id\" BLOB NOT NULL PRIMARY KEY, \"name\" TEXT NOT NULL);\nCREATE TABLE Post (\n    \"id\" BLOB NOT NULL PRIMARY KEY,\n    title TEXT NOT NULL,\n    body TEXT NOT NULL,\n    published INTEGER NOT NULL,\n    blog BLOB NOT NULL,\n    byline TEXT,\n    likes INTEGER NOT NULL,\n    FOREIGN KEY (blog) REFERENCES Blog(\"id\")\n);\nCREATE TABLE Post_tags_Many (\n    \"owner\" BLOB NOT NULL,\n    has TEXT NOT NULL,\n    FOREIGN KEY (\"owner\") REFERENCES Post(\"id\"),\n    FOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\")\n);\nCREATE TABLE \"Tag\" (\"tag\" TEXT NOT NULL PRIMARY KEY);\nCREATE TABLE IF NOT EXISTS butane_migrations (\"name\" TEXT NOT NULL PRIMARY KEY);\n"
      },
      "down": {
        "mysql": "ALTER TABLE Post DROP FOREIGN KEY Post_blog_fkey;\nALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_owner_fkey;\nALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_has_fkey;\nDROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE `Tag`;\n",
        "pg": "ALTER TABLE Post DROP CONSTRAINT Post_blog_fkey;\nALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_owner_fkey;\nALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_has_fkey;\nDROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\n",
        "sqlite": "DROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\n",
        "turso": "DROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\n"
      }
    },
    "20240406_035726416_tags": {
      "name": "20240406_035726416_tags",
      "db": {
        "tables": {
          "Blog": {
            "name": "Blog",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
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
              }
            ]
          },
          "Post": {
            "name": "Post",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": false,
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
                "name": "tags",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Json"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "blog",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Blob"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null,
                "reference": {
                  "Literal": {
                    "table_name": "Blog",
                    "column_name": "id"
                  }
                }
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
                "default": null
              },
              {
                "name": "likes",
                "sqltype": {
                  "KnownId": {
                    "Ty": "Int"
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
      "from": "20240401_095709389_init",
      "up": {
        "mysql": "ALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_owner_fkey;\nALTER TABLE Post_tags_Many DROP FOREIGN KEY Post_tags_Many_has_fkey;\nDROP TABLE Post_tags_Many;\nDROP TABLE `Tag`;\nALTER TABLE Post ADD COLUMN tags JSON NOT NULL DEFAULT null;\n",
        "pg": "ALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_owner_fkey;\nALTER TABLE Post_tags_Many DROP CONSTRAINT Post_tags_Many_has_fkey;\nDROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\nALTER TABLE Post ADD COLUMN tags JSONB NOT NULL DEFAULT null;\n",
        "sqlite": "DROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\nALTER TABLE Post ADD COLUMN tags TEXT NOT NULL DEFAULT null;\n",
        "turso": "DROP TABLE Post_tags_Many;\nDROP TABLE \"Tag\";\nALTER TABLE Post ADD COLUMN tags TEXT NOT NULL DEFAULT null;\n"
      },
      "down": {
        "mysql": "CREATE TABLE Post_tags_Many (\n`owner` VARBINARY(255) NOT NULL,\nhas VARCHAR(255) NOT NULL\n);\nCREATE TABLE `Tag` (\n`tag` VARCHAR(255) NOT NULL PRIMARY KEY\n);\nALTER TABLE Post DROP COLUMN tags;\nALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_owner_fkey FOREIGN KEY (`owner`) REFERENCES Post(`id`);\nALTER TABLE Post_tags_Many ADD CONSTRAINT Post_tags_Many_has_fkey FOREIGN KEY (has) REFERENCES `Tag`(`tag`);\n",
        "pg": "CREATE TABLE Post_tags_Many (\n\"owner\" BYTEA NOT NULL,\nhas TEXT NOT NULL\n);\nCREATE TABLE \"Tag\" (\n\"tag\" TEXT NOT NULL PRIMARY KEY\n);\nALTER TABLE Post DROP COLUMN tags;\nALTER TABLE Post_tags_Many ADD FOREIGN KEY (\"owner\") REFERENCES Post(\"id\");\nALTER TABLE Post_tags_Many ADD FOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\");\n",
        "sqlite": "CREATE TABLE Post_tags_Many (\n\"owner\" BLOB NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (\"owner\") REFERENCES Post(\"id\")\nFOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\")\n) STRICT;\nCREATE TABLE \"Tag\" (\n\"tag\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\nALTER TABLE Post DROP COLUMN tags;\n",
        "turso": "CREATE TABLE Post_tags_Many (\n    \"owner\" BLOB NOT NULL,\n    has TEXT NOT NULL,\n    FOREIGN KEY (\"owner\") REFERENCES Post(\"id\"),\n    FOREIGN KEY (has) REFERENCES \"Tag\"(\"tag\")\n);\nCREATE TABLE \"Tag\" (\"tag\" TEXT NOT NULL PRIMARY KEY);\nALTER TABLE Post DROP COLUMN tags;\n"
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
  "latest": "20240406_035726416_tags"
}"#;
    MemMigrations::from_json(json)
}
