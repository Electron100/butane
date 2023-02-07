//! Abstract representation of a database schema. If using the butane
//! CLI tool, there is no need to use this module. Even if applying
//! migrations without this tool, you are unlikely to need this module.

use crate::{Error, Result, SqlType, SqlVal};
use serde::{de::Deserializer, de::Visitor, ser::Serializer, Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Identifier for a type as used in a database column. Supports both
/// [SqlType](crate::SqlType) and identifiers known only by name. The
/// latter is used for custom types. `SqlType::Custom` cannot easily be used
/// directly at compile time when the proc macro serializing type information runs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TypeIdentifier {
    Ty(SqlType),
    Name(String),
}
impl From<SqlType> for TypeIdentifier {
    fn from(ty: SqlType) -> Self {
        TypeIdentifier::Ty(ty)
    }
}

/// Key used to help resolve `DeferredSqlType`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeKey {
    /// Represents a type which is the primary key for a table with the given name
    PK(String),
    /// Represents a type which is not natively known to butane but
    /// which butane will be made aware of with the `#\[butane_type\]` macro
    CustomType(String),
}
impl std::fmt::Display for TypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            TypeKey::PK(name) => write!(f, "PK({name})"),
            TypeKey::CustomType(name) => write!(f, "CustomType({name})"),
        }
    }
}
// Custom Serialize/Deserialize implementations so that it can be used
// as a string for HashMap keys, which are required to be strings (at least for serde_json)
impl serde::ser::Serialize for TypeKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&match self {
            TypeKey::PK(s) => format!("PK:{s}"),
            TypeKey::CustomType(s) => format!("CT:{s}"),
        })
    }
}
impl<'de> Deserialize<'de> for TypeKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<TypeKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(TypeKeyVisitor)
    }
}
struct TypeKeyVisitor;
impl<'de> Visitor<'de> for TypeKeyVisitor {
    type Value = TypeKey;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("serialized TypeKey")
    }
    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let rest = v.to_string().split_off(3);
        if v.starts_with("PK:") {
            Ok(TypeKey::PK(rest))
        } else if v.starts_with("CT:") {
            Ok(TypeKey::CustomType(rest))
        } else {
            Err(E::custom("Unknown type key string".to_string()))
        }
    }
}
impl PartialOrd for TypeKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TypeKey {
    fn cmp(&self, other: &Self) -> Ordering {
        use TypeKey::*;
        match self {
            PK(s) => match other {
                PK(other_s) => s.cmp(other_s),
                CustomType(_) => Ordering::Less,
            },
            CustomType(s) => match other {
                PK(_) => Ordering::Greater,
                CustomType(other_s) => s.cmp(other_s),
            },
        }
    }
}

#[derive(Debug)]
struct TypeResolver {
    // The types of some columns may not be known right away
    types: HashMap<TypeKey, TypeIdentifier>,
}
impl TypeResolver {
    fn new() -> Self {
        TypeResolver {
            types: HashMap::new(),
        }
    }
    fn find_type(&self, key: &TypeKey) -> Option<TypeIdentifier> {
        #[cfg(feature = "json")]
        if let TypeKey::CustomType(ct) = key {
            if ct.starts_with("HashMap < String,") {
                return Some(TypeIdentifier::from(SqlType::Json));
            }
        }
        self.types.get(key).cloned()
    }
    fn insert(&mut self, key: TypeKey, ty: TypeIdentifier) -> bool {
        use std::collections::hash_map::Entry;
        let entry = self.types.entry(key);
        match entry {
            Entry::Occupied(_) => false,
            Entry::Vacant(e) => {
                e.insert(ty);
                true
            }
        }
    }
    fn insert_pk(&mut self, key: &str, ty: TypeIdentifier) -> bool {
        self.insert(TypeKey::PK(key.to_string()), ty)
    }
}

/// Abstract representation of a database schema.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ADB {
    tables: HashMap<String, ATable>,
    extra_types: HashMap<TypeKey, DeferredSqlType>,
}
impl ADB {
    pub fn new() -> Self {
        ADB {
            tables: HashMap::new(),
            extra_types: HashMap::new(),
        }
    }
    pub fn tables(&self) -> impl Iterator<Item = &ATable> {
        self.tables.values()
    }
    pub fn get_table<'a>(&'a self, name: &str) -> Option<&'a ATable> {
        self.tables.get(name)
    }
    pub fn types(&self) -> &HashMap<TypeKey, DeferredSqlType> {
        &self.extra_types
    }
    pub fn replace_table(&mut self, table: ATable) {
        self.tables.insert(table.name.clone(), table);
    }
    pub fn remove_table(&mut self, name: &str) {
        self.tables.remove(name);
    }
    pub fn add_type(&mut self, key: TypeKey, sqltype: DeferredSqlType) {
        self.extra_types.insert(key, sqltype);
    }

    /// Fixup as many DeferredSqlType::Deferred instances as possible
    /// into DeferredSqlType::Known
    pub fn resolve_types(&mut self) -> Result<()> {
        let mut resolver = TypeResolver::new();
        let mut changed = true;
        while changed {
            changed = false;
            for table in &mut self.tables.values_mut() {
                if let Some(pk) = table.pk() {
                    let pktype = pk.typeid();
                    if let Ok(pktype) = pktype {
                        changed |= resolver.insert_pk(&table.name, pktype.clone());
                    }
                }
                for col in &mut table.columns {
                    changed |= col.resolve_type(&resolver);
                }
            }
            for (key, ty) in self.extra_types.iter_mut() {
                match ty {
                    DeferredSqlType::Known(ty) => {
                        changed |= resolver.insert(key.clone(), ty.clone().into()) || changed;
                    }
                    DeferredSqlType::KnownId(ty) => {
                        changed |= resolver.insert(key.clone(), ty.clone()) || changed;
                    }
                    DeferredSqlType::Deferred(tykey) => {
                        if let Some(sqltype) = resolver.find_type(tykey) {
                            *ty = sqltype.into();
                            changed = true;
                        }
                    }
                }
            }
        }

        // Now do a verification pass to ensure nothing is unresolved
        for table in &mut self.tables.values() {
            for col in &table.columns {
                if let DeferredSqlType::Deferred(key) = &col.sqltype {
                    return Err(Error::CannotResolveType(key.to_string()));
                }
            }
        }
        Ok(())
    }

    pub fn transform_with(&mut self, op: Operation) {
        use Operation::*;
        match op {
            AddTable(table) => {
                self.tables.insert(table.name.clone(), table);
            }
            AddTableIfNotExists(table) => {
                self.tables.insert(table.name.clone(), table);
            }
            RemoveTable(name) => self.remove_table(&name),
            AddColumn(table, col) => {
                if let Some(t) = self.tables.get_mut(&table) {
                    t.add_column(col);
                }
            }
            RemoveColumn(table, name) => {
                if let Some(t) = self.tables.get_mut(&table) {
                    t.remove_column(&name);
                }
            }
            ChangeColumn(table, _, new) => {
                if let Some(t) = self.tables.get_mut(&table) {
                    t.replace_column(new);
                }
            }
        }
    }
}

/// Abstract representation of a database table schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ATable {
    pub name: String,
    pub columns: Vec<AColumn>,
}
impl ATable {
    pub fn new(name: String) -> ATable {
        ATable {
            name,
            columns: Vec::new(),
        }
    }
    pub fn add_column(&mut self, col: AColumn) {
        self.replace_column(col);
    }
    pub fn column<'a>(&'a self, name: &str) -> Option<&'a AColumn> {
        self.columns.iter().find(|c| c.name == name)
    }
    pub fn replace_column(&mut self, col: AColumn) {
        if let Some(existing) = self.columns.iter_mut().find(|c| c.name == col.name) {
            *existing = col;
        } else {
            self.columns.push(col);
        }
    }
    pub fn remove_column(&mut self, name: &str) {
        self.columns.retain(|c| c.name != name);
    }
    pub fn pk(&self) -> Option<&AColumn> {
        self.columns.iter().find(|c| c.is_pk())
    }
}

/// SqlType which may not yet be known.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum DeferredSqlType {
    Known(SqlType), // Kept for backwards deserialization compat, supplanted by KnownId
    KnownId(TypeIdentifier),
    Deferred(TypeKey),
}
impl DeferredSqlType {
    fn resolve(&self, resolver: &TypeResolver) -> Result<TypeIdentifier> {
        match self {
            DeferredSqlType::KnownId(t) => Ok(t.clone()),
            DeferredSqlType::Known(t) => Ok(t.clone().into()),
            DeferredSqlType::Deferred(key) => resolver
                .find_type(key)
                .ok_or_else(|| crate::Error::UnknownSqlType(key.to_string())),
        }
    }
    fn is_known(&self) -> bool {
        match self {
            DeferredSqlType::Known(_) => true,
            DeferredSqlType::KnownId(_) => true,
            DeferredSqlType::Deferred(_) => false,
        }
    }
}
impl From<TypeIdentifier> for DeferredSqlType {
    fn from(id: TypeIdentifier) -> Self {
        DeferredSqlType::KnownId(id)
    }
}

/// Abstract representation of a database column schema.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AColumn {
    name: String,
    sqltype: DeferredSqlType,
    nullable: bool,
    pk: bool,
    auto: bool,
    #[serde(default)]
    unique: bool,
    default: Option<SqlVal>,
}
impl AColumn {
    pub fn new(
        name: impl Into<String>,
        sqltype: DeferredSqlType,
        nullable: bool,
        pk: bool,
        auto: bool,
        unique: bool,
        default: Option<SqlVal>,
    ) -> Self {
        AColumn {
            name: name.into(),
            sqltype,
            nullable,
            pk,
            auto,
            unique,
            default,
        }
    }
    /// Simple column that is non-null, non-auto, non-pk, non-unique with no default
    pub fn new_simple(name: impl Into<String>, sqltype: DeferredSqlType) -> Self {
        Self::new(name, sqltype, false, false, false, false, None)
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn nullable(&self) -> bool {
        self.nullable
    }
    pub fn unique(&self) -> bool {
        self.unique
    }
    pub fn is_pk(&self) -> bool {
        self.pk
    }
    pub fn default(&self) -> &Option<SqlVal> {
        &self.default
    }
    pub fn typeid(&self) -> Result<TypeIdentifier> {
        match &self.sqltype {
            DeferredSqlType::KnownId(t) => Ok(t.clone()),
            DeferredSqlType::Known(t) => Ok(t.clone().into()),
            DeferredSqlType::Deferred(t) => Err(crate::Error::UnknownSqlType(t.to_string())),
        }
    }
    /// Returns true if the type was previously unresolved but is now resolved
    fn resolve_type(&mut self, resolver: &'_ TypeResolver) -> bool {
        if self.sqltype.is_known() {
            // Already resolved, nothing to do
            false
        } else if let Ok(ty) = self.sqltype.resolve(resolver) {
            self.sqltype = DeferredSqlType::KnownId(ty);
            true
        } else {
            false
        }
    }
    pub fn is_auto(&self) -> bool {
        self.auto
    }
}

/// Individual operation use to apply a migration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    //future improvement: support column renames
    AddTable(ATable),
    AddTableIfNotExists(ATable),
    RemoveTable(String),
    AddColumn(String, AColumn),
    RemoveColumn(String, String),
    ChangeColumn(String, AColumn, AColumn),
}

/// Determine the operations necessary to move the database schema from `old` to `new`.
pub fn diff(old: &ADB, new: &ADB) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_names: HashSet<&String> = new.tables.keys().collect();
    let old_names: HashSet<&String> = old.tables.keys().collect();
    let new_tables = new_names.difference(&old_names);
    for added in new_tables {
        let added: &str = added.as_ref();
        ops.push(Operation::AddTable(
            new.tables.get(added).expect("no table").clone(),
        ));
    }
    for removed in old_names.difference(&new_names) {
        ops.push(Operation::RemoveTable((*removed).to_string()));
    }
    for table in new_names.intersection(&old_names) {
        let table: &str = table.as_ref();
        ops.append(&mut diff_table(
            old.tables.get(table).expect("no table"),
            new.tables.get(table).expect("no table"),
        ));
    }
    ops
}

fn col_by_name<'a>(columns: &'a [AColumn], name: &str) -> Option<&'a AColumn> {
    columns.iter().find(|c| c.name == name)
}

fn diff_table(old: &ATable, new: &ATable) -> Vec<Operation> {
    let mut ops: Vec<Operation> = Vec::new();
    let new_names: HashSet<&String> = new.columns.iter().map(|c| &c.name).collect();
    let old_names: HashSet<&String> = old.columns.iter().map(|c| &c.name).collect();
    let added_names = new_names.difference(&old_names);
    for added in added_names {
        let added: &str = added.as_ref();
        ops.push(Operation::AddColumn(
            new.name.clone(),
            col_by_name(&new.columns, added).unwrap().clone(),
        ));
    }
    for removed in old_names.difference(&new_names) {
        ops.push(Operation::RemoveColumn(
            old.name.clone(),
            (*removed).to_string(),
        ));
    }
    for colname in new_names.intersection(&old_names) {
        let colname: &str = colname.as_ref();
        let col = col_by_name(&new.columns, colname).unwrap();
        let old_col = col_by_name(&old.columns, colname).unwrap();
        if col == old_col {
            continue;
        }
        ops.push(Operation::ChangeColumn(
            new.name.clone(),
            old_col.clone(),
            col.clone(),
        ));
    }
    ops
}
