///! The contents of this module are primarily used by macro-generated
///! code. They are still part of Butane's public API and you are
///! welcome to use them, but be aware that you probably don't *need*
///! to.
use super::*;
pub trait DataObjectFields<T: DataObject> {
    // Since we don't have this yet
    // https://rust-lang.github.io/impl-trait-initiative/explainer/rpit_trait.html
    type IntoFieldsIter<'a>: IntoIterator<Item = &'a DataObjectFieldDef<T>>
    where
        Self: 'a,
        T: 'a;
    /// Allows iterating over all field definitions.
    fn field_defs(&'_ self) -> Self::IntoFieldsIter<'_>;
}

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
    pub fn name(&self) -> &str {
        self.name
    }
    pub fn is_nullable(&self) -> bool {
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
    pub fn sqltype(&self) -> &SqlType {
        &self.sqltype
    }
    pub fn is_auto(&self) -> bool {
        self.auto
    }
}
