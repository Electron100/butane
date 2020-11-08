pub use propane_codegen::{dataresult, model, propane_type};
pub use propane_core::db;
pub use propane_core::fkey::ForeignKey;
pub use propane_core::many::Many;
pub use propane_core::migrations;
pub use propane_core::pkey::{PrimaryKey, PrimaryKeyRef};
pub use propane_core::query;
pub use propane_core::{
    DataObject, DataResult, Error, FieldType, FromSql, IntoSql, ObjectState, Result, SqlType,
    SqlVal, ToSql,
};

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
/// * `like`: parameter is a SQL like expression string, e.g. `title.like("M%").
/// * `matches`: Parmeter is a sub-expression. Use with a
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
/// # use propane::query::BoolExpr;
/// # use propane_codegen::model;
/// # use propane_codegen::filter;
/// #[model]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
/// let e: BoolExpr = filter!(Contestant, nationality == "US" && rank < 42);
/// let firstplace = 1;
/// let e2 = filter!(Contestant, rank == { firstplace });
/// let e3 = filter!(Contestant, name.like("A%"));
///```
///
/// [`BoolExpr`]: crate::query::BoolExpr
/// [`Query`]: crate::query::Query
pub use propane_codegen::filter;

/// Constructs a filtered database query.
///
/// Use as `query!(Foo, expr)`, where `Foo` is a model type. Returns `[`Query`]`<Foo>`.
///
/// Shorthand for `Foo::query().filter(`[`filter`]`!(Foo, expr))`
//
/// # Examples
/// ```
/// # use propane::query::*;
/// # use propane_codegen::model;
/// # use propane::query;
/// # use propane::prelude::*;
/// #[model]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
/// let top_tier: Query<Contestant> = query!(Contestant, rank <= 10);
///```
///
/// [`filter]: crate::filter
/// [`Query`]: crate::query::Query
#[macro_export]
macro_rules! query {
    ($model:ident, $filter:expr) => {
        <$model as propane::DataResult>::query().filter(propane::filter!($model, $filter))
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
/// # use propane::db::ConnectionSpec;
/// # use propane::query::BoolExpr;
/// # use propane_codegen::model;
/// # use propane::prelude::*;
/// # use propane::query;
/// # use propane::find;
/// # use propane::DataObject;
/// #[model]
/// struct Contestant {
///   #[pk]
///   name: String,
///   rank: i32,
///   nationality: String
/// }
///
/// let conn = propane::db::connect(&ConnectionSpec::new("sqlite", "foo.db")).unwrap();
/// let alice: Result<Contestant, propane::Error> = find!(Contestant, name == "Alice", &conn);
///```
///
/// [`filter]: crate::filter
/// [`Result`]: crate::Result
/// [`NoSuchObject`]: crate::Error::NoSuchObject
#[macro_export]
macro_rules! find {
    ($dbobj:ident, $filter:expr, $conn:expr) => {
        propane::query!($dbobj, $filter)
            .limit(1)
            .load($conn)
            .and_then(|mut results| results.pop().ok_or(propane::Error::NoSuchObject))
    };
}

pub mod prelude {
    //! Prelude module to improve ergonomics.
    //!
    //! Its use is recommended, but not required. If not used, the use
    //! of propane's macros may require some of its re-exports to be
    //! used manually.
    #[doc(no_inline)]
    pub use crate::DataObject;
    #[doc(no_inline)]
    pub use crate::DataResult;
    pub use propane_core::db::BackendConnection;
}
