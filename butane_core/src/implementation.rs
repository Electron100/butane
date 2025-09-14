//! The contents of this module are primarily used by macro-generated
//! code. They are still part of Butane's public API and you are
//! welcome to use them, but be aware that you probably don't *need*
//! to.
#![deny(missing_docs)]

use super::*;

/// Trait implemented by `[DataObject::Fields] which allows retrieving the definitions of all fields in a data object.
pub trait DataObjectFields {
    /// Corresponding object type.
    type DBO: DataObject;
    /// Helper type for `field_defs()` return type. Since we don't have this yet
    // https://rust-lang.github.io/impl-trait-initiative/explainer/rpit_trait.html
    type IntoFieldsIter<'a>: IntoIterator<Item = &'a DataObjectFieldDef<Self::DBO>>
    where
        Self: 'a,
        Self::DBO: 'a;
    /// Allows iterating over all field definitions.
    fn field_defs(&'_ self) -> Self::IntoFieldsIter<'_>;
}

/// Definition for a single [DataObject] field.
#[derive(Clone, Debug, PartialEq, TypedBuilder)]
pub struct DataObjectFieldDef<T: DataObject> {
    name: &'static str,
    sqltype: SqlType,
    nullable: bool,
    #[builder(default = false)]
    pk: bool,
    #[builder(default = false)]
    auto: bool,
    #[builder(default = false)]
    unique: bool,
    #[builder(default)]
    default: Option<SqlVal>,
    #[builder(default)]
    phantom: PhantomData<T>,
}
impl<T: DataObject> DataObjectFieldDef<T> {
    /// Returns the name of the field.
    pub fn name(&self) -> &str {
        self.name
    }
    /// Returns whether the field is nullable.
    pub fn is_nullable(&self) -> bool {
        self.nullable
    }
    /// Returns whether values of the field must be unique.
    pub fn is_unique(&self) -> bool {
        self.unique
    }
    /// Returns whether the field is a primary key.
    pub fn is_pk(&self) -> bool {
        self.pk
    }
    /// Returns the default value for the field, if any.
    pub fn default(&self) -> Option<&SqlVal> {
        self.default.as_ref()
    }
    /// Returns the sqltype of the field.
    pub fn sqltype(&self) -> &SqlType {
        &self.sqltype
    }
    /// Returns whether the field is auto-valued.
    pub fn is_auto(&self) -> bool {
        self.auto
    }
}
