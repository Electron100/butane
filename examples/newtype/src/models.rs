//! Models for the newtype example.

use butane::AutoPk;
use butane::{model, FieldType};
use serde::{Deserialize, Serialize};

/// Newtype Patch wrapping [json_patch::Patch].
#[derive(Debug, Deserialize, FieldType, Serialize)]
pub struct Patch(json_patch::Patch);

impl Default for Patch {
    /// Create a new empty Patch.
    /// https://github.com/idubrov/json-patch/pull/32 will make this unnecessary.
    fn default() -> Self {
        Patch(json_patch::Patch(vec![]))
    }
}

/// Main table.
#[model]
#[derive(Debug, Default)]
pub struct Record {
    /// Id of the record.
    pub id: AutoPk<i64>,
    /// Data to be stored.
    pub patch: Patch,
}

impl Record {
    /// Create a new Record.
    pub fn new(patch: Patch) -> Self {
        Record {
            patch,
            ..Default::default()
        }
    }
}
