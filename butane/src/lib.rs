//! An experimental ORM for Rust with a focus on simplicity and on writing Rust, not SQL

//! Butane takes an object-oriented approach to database operations.
//! It may be thought of as much as an object-persistence system as an ORM.
//! The fact that it is backed by a SQL database is mostly an implementation detail to the API consumer.

#![deny(missing_docs)]
pub use butane_codegen::{butane_type, dataresult, model, FieldType};
pub use butane_core::custom;
pub use butane_core::fkey::ForeignKey;
pub use butane_core::many::Many;
pub use butane_core::migrations;
pub use butane_core::query;
pub use butane_core::{
    AsPrimaryKey, DataObject, DataResult, Error, FieldType, FromSql, ObjectState, PrimaryKeyType,
    Result, SqlType, SqlVal, SqlValRef, ToSql,
};

pub mod db {
    //! Database helpers
    pub use butane_core::db::*;
}

/// Macro to construct a [`BoolExpr`] (for use with a [`Query`]) from
/// an expression with Rust syntax.
///
/// Using this macro instead of constructing a `BoolExpr` has two
/// advantages:
/// 1. It will generally be more ergonomic
/// 2. References to nonexistent fields or type mismatches
///    (e.g. comparing a number to a string) will generate a compilation error
///
/// Usage: `filter!(Foo, expr)` where `Foo` is a model type (with the
/// `#[model]` attribute applied) and `expr` is a Rust-like expression
/// with a boolean value. `Foo`'s fields may be referred to as if they
/// were variables.
///
/// # Rust values
/// To refer to values from the surrounding rust function, enclose
/// them in braces, like `filter!(Foo, bar == {bar})`
///
/// # Function-like operations
/// Filters support some operations for which Rust does not have operators and which are instead
/// represented syntactically as function calls.
/// * `like`: parameter is a SQL LIKE expression string, e.g. `title.like("M%").
/// * `matches`: Parameter is a sub-expression. Use with a
///   [`ForeignKey`] field to evaluate as true if the referent
///   matches. For example, to find all posts made in blogs by people
///   named "Pete" we might say `filter!(Post, `blog.matches(author == "Pete"))`.
/// * `contains`: Essentially the many-to-many version of `matches`.
///    Parameter is a sub-expression. Use with a [`Many`]
///    field to evaluate as true if one of the many referents matches
///    the given expression. For example, in a blog post model with a field
///    `tags: Many<Tag>` we could filter to posts with a "cats" with
///    the following `tags.contains(tag == "cats"). If the expression
///    is single literal, it is assumed to be used to match the
///    primary key.
///
/// # Examples
/// ```
/// # use butane::query::BoolExpr;
/// # use butane_codegen::model;
/// # use butane_codegen::filter;
/// #[model]
/// #[derive(Clone)]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
/// let e: BoolExpr = filter!(Contestant, nationality == "US" && rank < 42);
/// let first_place = 1;
/// let e2 = filter!(Contestant, rank == { first_place });
/// let e3 = filter!(Contestant, name.like("A%"));
/// ```
///
/// [`BoolExpr`]: crate::query::BoolExpr
/// [`Query`]: crate::query::Query
pub use butane_codegen::filter;

/// Constructs a filtered database query.
///
/// Use as `query!(Foo, expr)`, where `Foo` is a model type. Returns [`Query`]`<Foo>`.
///
/// Shorthand for `Foo::query().filter(`[`filter`]`!(Foo, expr))`
//
/// # Examples
/// ```
/// # use butane::query::*;
/// # use butane_codegen::model;
/// # use butane::query;
/// # use butane::prelude::*;
/// #[model]
/// #[derive(Clone)]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
/// let top_tier: Query<Contestant> = query!(Contestant, rank <= 10);
/// ```
///
/// [`filter]: crate::filter
/// [`Query`]: crate::query::Query
#[macro_export]
macro_rules! query {
    ($model:ident, $filter:expr) => {
        <$model as butane::DataResult>::query().filter(butane::filter!($model, $filter))
    };
}

/// Type-safe way to refer to a column name. Use as
/// `colname!(MODEL_TYPE, FIELD_NAME)`. E.g. For a model type `Foo`
/// with a field `bar`, `colname!(Foo, bar) would return `"bar"`, but
/// `colname!(Foo, bat)` would be a compiler error (assuming `Foo`
/// does not have such a field.
#[macro_export]
macro_rules! colname {
    ($model:ident, $col:ident) => {
        $model::fields().$col().name()
    };
}

/// Finds a specific database object.
///
/// Use as `find!(Foo, expr, conn)`, where `Foo` is a model type and
/// conn implements `ConnectionImpl`. Returns
/// [`Result`]`<`Foo`>`. The error will be [`NoSuchObject`] if no
/// object was found. If more than one object matches the expression,
/// the first one found is returned.
///
/// This macro is for convenience -- it does nothing that can't be done with `query!` or `filter!`.
///
/// # Examples
/// ```no_run
/// # use butane::db::ConnectionSpec;
/// # use butane::query::BoolExpr;
/// # use butane_codegen::model;
/// # use butane::prelude::*;
/// # use butane::query;
/// # use butane::find;
/// # use butane::DataObject;
/// #[model]
/// #[derive(Clone)]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
///
/// let conn = butane::db::connect(&ConnectionSpec::new("sqlite", "foo.db")).unwrap();
/// let alice: Result<Contestant, butane::Error> = find!(Contestant, name == "Alice", &conn);
/// ```
///
/// [`filter]: crate::filter
/// [`Result`]: crate::Result
/// [`NoSuchObject`]: crate::Error::NoSuchObject
#[macro_export]
macro_rules! find {
    ($dbobj:ident, $filter:expr, $conn:expr) => {
        butane::query!($dbobj, $filter)
            .limit(1)
            .load($conn)
            .and_then(|mut results| results.pop().ok_or(butane::Error::NoSuchObject))
    };
}

pub mod prelude {
    //! Prelude module to improve ergonomics.
    //!
    //! Its use is recommended, but not required. If not used, the use
    //! of butane's macros may require some of its re-exports to be
    //! used manually.
    #[doc(no_inline)]
    pub use crate::DataObject;
    #[doc(no_inline)]
    pub use crate::DataResult;
    pub use butane_core::db::BackendConnection;
}
