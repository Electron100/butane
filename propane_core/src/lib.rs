use failure;

pub mod adb;
pub mod db;
pub mod migrations;

pub use adb::*;

pub type Result<T> = std::result::Result<T, failure::Error>;
