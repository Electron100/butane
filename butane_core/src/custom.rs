//! For supporting additional types with the Pg backend (or other
//! future backends).
//!
//! For an example of usage, see `butane/tests/custom_pg.rs` in the
//! source repository. Not supported for the Sqlite backend as Sqlite
//! supports a very limited set of types to begin with.

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio_postgres as postgres;

/// For use with [SqlType::Custom](crate::SqlType)
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SqlTypeCustom {
    #[cfg(feature = "pg")]
    Pg(#[serde(with = "pgtypeser")] tokio_postgres::types::Type),
}

/// For use with [SqlVal::Custom](crate::SqlVal)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SqlValCustom {
    #[cfg(feature = "pg")]
    Pg {
        #[serde(with = "pgtypeser")]
        ty: postgres::types::Type,
        data: Vec<u8>,
    },
}

impl SqlValCustom {
    pub fn as_valref(&self) -> SqlValRefCustom {
        match self {
            #[cfg(feature = "pg")]
            SqlValCustom::Pg { ty, data } => SqlValRefCustom::PgBytes {
                ty: ty.clone(),
                data: data.as_ref(),
            },
            #[cfg(not(feature = "pg"))]
            _ => panic!("SqlValCustom unsupported"),
        }
    }
}

impl fmt::Display for SqlValCustom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            #[cfg(feature = "pg")]
            SqlValCustom::Pg { ty, .. } => {
                f.write_str(&format!("<custom PG value of type {}>", ty))
            }
            #[cfg(not(feature = "pg"))]
            _ => f.write_str("<unknown custom value>"),
        }
    }
}

#[cfg(feature = "pg")]
impl postgres::types::ToSql for SqlValCustom {
    fn to_sql(
        &self,
        wanted_ty: &postgres::types::Type,
        out: &mut bytes::BytesMut,
    ) -> std::result::Result<
        postgres::types::IsNull,
        Box<dyn std::error::Error + 'static + Sync + Send>,
    > {
        use bytes::BufMut;
        match self {
            SqlValCustom::Pg { ty, data } => {
                if ty != wanted_ty {
                    return Err(Box::new(crate::Error::Internal(format!(
                        "postgres type mismatch. Wanted {} but have {}",
                        wanted_ty, ty
                    ))));
                }
                out.put(data.as_ref())
            }
        }
        Ok(postgres::types::IsNull::No)
    }

    fn accepts(_: &postgres::types::Type) -> bool {
        // Unfortunately, this is a type method rather than an instance method,
        // so we don't know what this specific instance accepts :(
        true
    }
    postgres::types::to_sql_checked!();
}

/// For use with [SqlValRef::Custom](crate::SqlValRef)
#[derive(Clone, Debug)]
pub enum SqlValRefCustom<'a> {
    /// Used with Postgres, but suitable only for input (e.g. input to
    /// a query), not suitable for parsing in a [FromSql](crate::FromSql) implementation.
    #[cfg(feature = "pg")]
    PgToSql {
        ty: postgres::types::Type,
        tosql: &'a (dyn postgres::types::ToSql + Sync),
    },
    /// The Pg backend will return SqlValRef instances of this
    /// type. May also be used by [ToSql](crate::ToSql)
    /// implementations, but may be less convenient than `PgToSql` for that purpose.
    #[cfg(feature = "pg")]
    PgBytes {
        ty: postgres::types::Type,
        data: &'a [u8],
    },
    #[cfg(not(feature = "pg"))]
    Phantom(std::marker::PhantomData<&'a ()>),
}

impl From<SqlValRefCustom<'_>> for SqlValCustom {
    fn from(r: SqlValRefCustom) -> SqlValCustom {
        match r {
            #[cfg(feature = "pg")]
            SqlValRefCustom::PgToSql { ty, tosql } => {
                let mut b = bytes::BytesMut::new();
                // TODO avoid unwrap
                tosql.to_sql_checked(&ty, &mut b).unwrap();
                SqlValCustom::Pg {
                    ty,
                    data: b.to_vec(),
                }
            }
            #[cfg(feature = "pg")]
            SqlValRefCustom::PgBytes { ty, data } => SqlValCustom::Pg {
                ty,
                data: data.into(),
            },
            #[cfg(not(feature = "pg"))]
            SqlValRefCustom::Phantom(_) => {
                panic!("phantom SqlValRefCustom should not be instantiated")
            }
        }
    }
}

#[cfg(feature = "pg")]
mod pgtypeser {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use tokio_postgres as postgres;

    pub fn serialize<S>(ty: &postgres::types::Type, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let ty = SerializablePgType::from(ty.clone());
        ty.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<postgres::types::Type, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(SerializablePgType::deserialize(deserializer)?.into())
    }

    //Serializable version of postgres::types::Type
    #[derive(Serialize, Deserialize, Clone)]
    struct SerializablePgType {
        name: String,
        oid: u32,
        kind: Box<SerializablePgKind>,
        schema: String,
    }
    impl From<postgres::types::Type> for SerializablePgType {
        fn from(ty: postgres::types::Type) -> Self {
            Self {
                name: ty.name().to_string(),
                oid: ty.oid(),
                kind: Box::new(ty.kind().clone().into()),
                schema: ty.schema().to_string(),
            }
        }
    }
    impl From<SerializablePgType> for postgres::types::Type {
        fn from(spt: SerializablePgType) -> postgres::types::Type {
            postgres::types::Type::new(spt.name, spt.oid, (*spt.kind).into(), spt.schema)
        }
    }

    #[derive(Serialize, Deserialize, Clone)]
    enum SerializablePgKind {
        Simple,
        Enum(Vec<String>),
        Pseudo,
        Array(SerializablePgType),
        Range(SerializablePgType),
        Domain(SerializablePgType),
        Composite(Vec<SerializablePgField>),
    }
    impl From<postgres::types::Kind> for SerializablePgKind {
        fn from(k: postgres::types::Kind) -> Self {
            use postgres::types::Kind::*;
            match k {
                Simple => Self::Simple,
                Enum(v) => Self::Enum(v),
                Pseudo => Self::Pseudo,
                Array(ty) => Self::Array(ty.into()),
                Range(ty) => Self::Range(ty.into()),
                Domain(ty) => Self::Domain(ty.into()),
                Composite(v) => Self::Composite(v.into_iter().map(|f| f.into()).collect()),
                // TODO why is rustc requiring wildcard here
                _ => panic!("Unhandled variant"),
            }
        }
    }
    impl From<SerializablePgKind> for postgres::types::Kind {
        fn from(spk: SerializablePgKind) -> postgres::types::Kind {
            use postgres::types::Kind::*;
            match spk {
                SerializablePgKind::Simple => Simple,
                SerializablePgKind::Enum(v) => Enum(v),
                SerializablePgKind::Pseudo => Pseudo,
                SerializablePgKind::Array(ty) => Array(ty.into()),
                SerializablePgKind::Range(ty) => Range(ty.into()),
                SerializablePgKind::Domain(ty) => Domain(ty.into()),
                SerializablePgKind::Composite(v) => {
                    Composite(v.into_iter().map(|f| f.into()).collect())
                }
            }
        }
    }

    #[derive(Serialize, Deserialize, Clone)]
    struct SerializablePgField {
        name: String,
        ty: SerializablePgType,
    }
    impl From<postgres::types::Field> for SerializablePgField {
        fn from(f: postgres::types::Field) -> Self {
            Self {
                name: f.name().to_string(),
                ty: f.type_().clone().into(),
            }
        }
    }
    impl From<SerializablePgField> for postgres::types::Field {
        fn from(spf: SerializablePgField) -> postgres::types::Field {
            postgres::types::Field::new(spf.name, spf.ty.into())
        }
    }
}
