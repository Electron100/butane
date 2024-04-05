//! Butane migrations embedded in Rust.

use butane::migrations::MemMigrations;

/// Load the butane migrations embedded in Rust.
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {
    let json = r#"{
  "migrations": {
    "20201229_144636751_init": {
      "name": "20201229_144636751_init",
      "db": {
        "tables": {
          "Blog": {
            "name": "Blog",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "Known": "BigInt"
                },
                "nullable": false,
                "pk": true,
                "auto": true,
                "unique": false,
                "default": null
              },
              {
                "name": "name",
                "sqltype": {
                  "Known": "Text"
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
                  "Known": "Int"
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
                  "Known": "Text"
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
                  "Known": "Text"
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
                  "Known": "Bool"
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
                  "Known": "BigInt"
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
                  "Known": "Text"
                },
                "nullable": true,
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
                  "Known": "Int"
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "has",
                "sqltype": {
                  "Known": "Text"
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              }
            ]
          },
          "Tag": {
            "name": "Tag",
            "columns": [
              {
                "name": "tag",
                "sqltype": {
                  "Known": "Text"
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
        "pg": "CREATE TABLE Blog (\nid BIGSERIAL NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nCREATE TABLE Post (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT \n);\nCREATE TABLE Post_tags_Many (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nCREATE TABLE Tag (\ntag TEXT NOT NULL PRIMARY KEY\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE Blog (\nid INTEGER NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nCREATE TABLE Post (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT\n);\nCREATE TABLE Post_tags_Many (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nCREATE TABLE Tag (\ntag TEXT NOT NULL PRIMARY KEY\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n"
      },
      "down": {
        "pg": "DROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE Tag;\n",
        "sqlite": "DROP TABLE Blog;\nDROP TABLE Post;\nDROP TABLE Post_tags_Many;\nDROP TABLE Tag;\n"
      }
    },
    "20201229_171630604_likes": {
      "name": "20201229_171630604_likes",
      "db": {
        "tables": {
          "Blog": {
            "name": "Blog",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "Known": "BigInt"
                },
                "nullable": false,
                "pk": true,
                "auto": true,
                "unique": false,
                "default": null
              },
              {
                "name": "name",
                "sqltype": {
                  "Known": "Text"
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
                  "Known": "Int"
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
                  "Known": "Text"
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
                  "Known": "Text"
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
                  "Known": "Bool"
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
                  "Known": "BigInt"
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
                  "Known": "Text"
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
                  "Known": "Int"
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
                  "Known": "Int"
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "has",
                "sqltype": {
                  "Known": "Text"
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              }
            ]
          },
          "Tag": {
            "name": "Tag",
            "columns": [
              {
                "name": "tag",
                "sqltype": {
                  "Known": "Text"
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
      "from": "20201229_144636751_init",
      "up": {
        "pg": "ALTER TABLE Post ADD COLUMN likes INTEGER NOT NULL DEFAULT 0;\n",
        "sqlite": "ALTER TABLE Post ADD COLUMN likes INTEGER NOT NULL DEFAULT 0;\n"
      },
      "down": {
        "pg": "ALTER TABLE Post DROP COLUMN likes;\n",
        "sqlite": "CREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\n"
      }
    },
    "20240115_023841384_dbconstraints": {
      "name": "20240115_023841384_dbconstraints",
      "db": {
        "tables": {
          "Blog": {
            "name": "Blog",
            "columns": [
              {
                "name": "id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "BigInt"
                  }
                },
                "nullable": false,
                "pk": true,
                "auto": true,
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
                "name": "blog",
                "sqltype": {
                  "KnownId": {
                    "Ty": "BigInt"
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
      "from": "20201229_171630604_likes",
      "up": {
        "pg": "CREATE TABLE Blog__butane_tmp (\nid BIGSERIAL NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Blog__butane_tmp (\nid BIGSERIAL NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nALTER TABLE Post__butane_tmp ADD FOREIGN KEY (blog) REFERENCES Blog(id);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nALTER TABLE Post_tags_Many__butane_tmp ADD FOREIGN KEY (has) REFERENCES Tag(tag);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nALTER TABLE Post_tags_Many__butane_tmp ADD FOREIGN KEY (owner) REFERENCES Post(id);\nALTER TABLE Post_tags_Many__butane_tmp ADD FOREIGN KEY (has) REFERENCES Tag(tag);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Tag__butane_tmp (\ntag TEXT NOT NULL PRIMARY KEY\n);\nINSERT INTO Tag__butane_tmp SELECT tag FROM Tag;\nDROP TABLE Tag;\nALTER TABLE Tag__butane_tmp RENAME TO Tag;\n",
        "sqlite": "CREATE TABLE Blog__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Blog__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL,\nFOREIGN KEY (blog) REFERENCES Blog(id)\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (has) REFERENCES Tag(tag)\n);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (owner) REFERENCES Post(id)\nFOREIGN KEY (has) REFERENCES Tag(tag)\n);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Tag__butane_tmp (\ntag TEXT NOT NULL PRIMARY KEY\n);\nINSERT INTO Tag__butane_tmp SELECT tag FROM Tag;\nDROP TABLE Tag;\nALTER TABLE Tag__butane_tmp RENAME TO Tag;\n"
      },
      "down": {
        "pg": "CREATE TABLE Blog__butane_tmp (\nid BIGSERIAL NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Blog__butane_tmp (\nid BIGSERIAL NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid SERIAL NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished BOOLEAN NOT NULL,\nblog BIGINT NOT NULL,\nbyline TEXT ,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nALTER TABLE Post_tags_Many__butane_tmp ADD FOREIGN KEY (owner) REFERENCES Post(id);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Tag__butane_tmp (\ntag TEXT NOT NULL PRIMARY KEY\n);\nINSERT INTO Tag__butane_tmp SELECT tag FROM Tag;\nDROP TABLE Tag;\nALTER TABLE Tag__butane_tmp RENAME TO Tag;\n",
        "sqlite": "CREATE TABLE Blog__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Blog__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL\n);\nINSERT INTO Blog__butane_tmp SELECT id, \"name\" FROM Blog;\nDROP TABLE Blog;\nALTER TABLE Blog__butane_tmp RENAME TO Blog;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post__butane_tmp (\nid INTEGER NOT NULL PRIMARY KEY,\ntitle TEXT NOT NULL,\nbody TEXT NOT NULL,\npublished INTEGER NOT NULL,\nblog INTEGER NOT NULL,\nbyline TEXT,\nlikes INTEGER NOT NULL\n);\nINSERT INTO Post__butane_tmp SELECT id, title, body, published, blog, byline, likes FROM Post;\nDROP TABLE Post;\nALTER TABLE Post__butane_tmp RENAME TO Post;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL,\nFOREIGN KEY (owner) REFERENCES Post(id)\n);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Post_tags_Many__butane_tmp (\nowner INTEGER NOT NULL,\nhas TEXT NOT NULL\n);\nINSERT INTO Post_tags_Many__butane_tmp SELECT owner, has FROM Post_tags_Many;\nDROP TABLE Post_tags_Many;\nALTER TABLE Post_tags_Many__butane_tmp RENAME TO Post_tags_Many;\nCREATE TABLE Tag__butane_tmp (\ntag TEXT NOT NULL PRIMARY KEY\n);\nINSERT INTO Tag__butane_tmp SELECT tag FROM Tag;\nDROP TABLE Tag;\nALTER TABLE Tag__butane_tmp RENAME TO Tag;\n"
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
  "latest": "20240115_023841384_dbconstraints"
}"#;
    MemMigrations::from_json(json)
}
