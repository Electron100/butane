//! Butane migrations embedded in Rust.

use butane::migrations::MemMigrations;

/// Load the butane migrations embedded in Rust.
pub fn get_migrations() -> Result<MemMigrations, butane::Error> {
    let json = r#"{
  "migrations": {
    "20251031_014308910_init": {
      "name": "20251031_014308910_init",
      "db": {
        "tables": {
          "Config": {
            "name": "Config",
            "columns": [
              {
                "name": "key",
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
                "name": "value",
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
                "name": "description",
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
              }
            ]
          },
          "Order": {
            "name": "Order",
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
                "name": "order_number",
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
                "name": "user",
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
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              },
              {
                "name": "product",
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
                    "table_name": "Product",
                    "column_name": "sku"
                  }
                }
              },
              {
                "name": "quantity",
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
              },
              {
                "name": "status",
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
          "Product": {
            "name": "Product",
            "columns": [
              {
                "name": "sku",
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
                "name": "category",
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
                "name": "price_cents",
                "sqltype": {
                  "KnownId": {
                    "Ty": "BigInt"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "in_stock",
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
                "name": "username",
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
              },
              {
                "name": "display_name",
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
                "name": "status",
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
        "pg": "CREATE TABLE Config (\n\"key\" TEXT NOT NULL PRIMARY KEY,\n\"value\" TEXT NOT NULL,\ndescription TEXT\n);\nCREATE TABLE \"Order\" (\n\"id\" BIGSERIAL NOT NULL PRIMARY KEY,\norder_number TEXT NOT NULL,\n\"user\" BIGINT NOT NULL,\nproduct TEXT NOT NULL,\nquantity INTEGER NOT NULL,\n\"status\" TEXT NOT NULL\n);\nCREATE TABLE Product (\nsku TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\ncategory TEXT NOT NULL,\nprice_cents BIGINT NOT NULL,\nin_stock BOOLEAN NOT NULL\n);\nCREATE TABLE \"User\" (\n\"id\" BIGSERIAL NOT NULL PRIMARY KEY,\nusername TEXT NOT NULL,\nemail TEXT NOT NULL,\ndisplay_name TEXT,\n\"status\" TEXT NOT NULL\n);\nALTER TABLE \"Order\" ADD FOREIGN KEY (\"user\") REFERENCES \"User\"(\"id\");\nALTER TABLE \"Order\" ADD FOREIGN KEY (product) REFERENCES Product(sku);\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n);\n",
        "sqlite": "CREATE TABLE Config (\n\"key\" TEXT NOT NULL PRIMARY KEY,\n\"value\" TEXT NOT NULL,\ndescription TEXT\n) STRICT;\nCREATE TABLE \"Order\" (\n\"id\" INTEGER NOT NULL PRIMARY KEY,\norder_number TEXT NOT NULL,\n\"user\" INTEGER NOT NULL,\nproduct TEXT NOT NULL,\nquantity INTEGER NOT NULL,\n\"status\" TEXT NOT NULL,\nFOREIGN KEY (\"user\") REFERENCES \"User\"(\"id\")\nFOREIGN KEY (product) REFERENCES Product(sku)\n) STRICT;\nCREATE TABLE Product (\nsku TEXT NOT NULL PRIMARY KEY,\n\"name\" TEXT NOT NULL,\ncategory TEXT NOT NULL,\nprice_cents INTEGER NOT NULL,\nin_stock INTEGER NOT NULL\n) STRICT;\nCREATE TABLE \"User\" (\n\"id\" INTEGER NOT NULL PRIMARY KEY,\nusername TEXT NOT NULL,\nemail TEXT NOT NULL,\ndisplay_name TEXT,\n\"status\" TEXT NOT NULL\n) STRICT;\nCREATE TABLE IF NOT EXISTS butane_migrations (\n\"name\" TEXT NOT NULL PRIMARY KEY\n) STRICT;\n",
        "turso": "CREATE TABLE Config (\"key\" TEXT NOT NULL PRIMARY KEY, \"value\" TEXT NOT NULL, description TEXT);\nCREATE TABLE \"Order\" (\n    \"id\" INTEGER NOT NULL PRIMARY KEY,\n    order_number TEXT NOT NULL,\n    \"user\" INTEGER NOT NULL,\n    product TEXT NOT NULL,\n    quantity INTEGER NOT NULL,\n    \"status\" TEXT NOT NULL,\n    FOREIGN KEY (\"user\") REFERENCES \"User\"(\"id\"),\n    FOREIGN KEY (product) REFERENCES Product(sku)\n);\nCREATE TABLE Product (\n    sku TEXT NOT NULL PRIMARY KEY,\n    \"name\" TEXT NOT NULL,\n    category TEXT NOT NULL,\n    price_cents INTEGER NOT NULL,\n    in_stock INTEGER NOT NULL\n);\nCREATE TABLE \"User\" (\n    \"id\" INTEGER NOT NULL PRIMARY KEY,\n    username TEXT NOT NULL,\n    email TEXT NOT NULL,\n    display_name TEXT,\n    \"status\" TEXT NOT NULL\n);\nCREATE TABLE IF NOT EXISTS butane_migrations (\"name\" TEXT NOT NULL PRIMARY KEY);\n"
      },
      "down": {
        "pg": "ALTER TABLE \"Order\" DROP CONSTRAINT \"Order_user_fkey\";\nALTER TABLE \"Order\" DROP CONSTRAINT \"Order_product_fkey\";\nDROP TABLE Config;\nDROP TABLE \"Order\";\nDROP TABLE Product;\nDROP TABLE \"User\";\n",
        "sqlite": "DROP TABLE Config;\nDROP TABLE \"Order\";\nDROP TABLE Product;\nDROP TABLE \"User\";\n",
        "turso": "DROP TABLE Config;\nDROP TABLE \"Order\";\nDROP TABLE Product;\nDROP TABLE \"User\";\n"
      }
    },
    "20251031_015240522_add_sessions": {
      "name": "20251031_015240522_add_sessions",
      "db": {
        "tables": {
          "Config": {
            "name": "Config",
            "columns": [
              {
                "name": "key",
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
                "name": "value",
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
                "name": "description",
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
              }
            ]
          },
          "Order": {
            "name": "Order",
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
                "name": "order_number",
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
                "name": "user",
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
                    "table_name": "User",
                    "column_name": "id"
                  }
                }
              },
              {
                "name": "product",
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
                    "table_name": "Product",
                    "column_name": "sku"
                  }
                }
              },
              {
                "name": "quantity",
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
              },
              {
                "name": "status",
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
          "Product": {
            "name": "Product",
            "columns": [
              {
                "name": "sku",
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
                "name": "category",
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
                "name": "price_cents",
                "sqltype": {
                  "KnownId": {
                    "Ty": "BigInt"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "in_stock",
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
              }
            ]
          },
          "Session": {
            "name": "Session",
            "columns": [
              {
                "name": "session_id",
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
                "name": "user_id",
                "sqltype": {
                  "KnownId": {
                    "Ty": "BigInt"
                  }
                },
                "nullable": false,
                "pk": false,
                "auto": false,
                "unique": false,
                "default": null
              },
              {
                "name": "ip_address",
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
                "name": "user_agent",
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
                "name": "status",
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
                "name": "device_fingerprint",
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
                "name": "username",
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
              },
              {
                "name": "display_name",
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
                "name": "status",
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
      "from": "20251031_014308910_init",
      "up": {
        "pg": "CREATE TABLE \"Session\" (\nsession_id TEXT NOT NULL PRIMARY KEY,\nuser_id BIGINT NOT NULL,\nip_address TEXT NOT NULL,\nuser_agent TEXT NOT NULL,\n\"status\" TEXT NOT NULL,\ndevice_fingerprint TEXT\n);\n",
        "sqlite": "CREATE TABLE \"Session\" (\nsession_id TEXT NOT NULL PRIMARY KEY,\nuser_id INTEGER NOT NULL,\nip_address TEXT NOT NULL,\nuser_agent TEXT NOT NULL,\n\"status\" TEXT NOT NULL,\ndevice_fingerprint TEXT\n) STRICT;\n",
        "turso": "CREATE TABLE \"Session\" (\n    session_id TEXT NOT NULL PRIMARY KEY,\n    user_id INTEGER NOT NULL,\n    ip_address TEXT NOT NULL,\n    user_agent TEXT NOT NULL,\n    \"status\" TEXT NOT NULL,\n    device_fingerprint TEXT\n);\n"
      },
      "down": {
        "pg": "DROP TABLE \"Session\";\n",
        "sqlite": "DROP TABLE \"Session\";\n",
        "turso": "DROP TABLE \"Session\";\n"
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
  "latest": "20251031_015240522_add_sessions"
}"#;
    MemMigrations::from_json(json)
}
