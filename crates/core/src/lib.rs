//! Core domain logic for the Devcontainer CLI.

pub mod config;
pub mod errors;
pub mod lifecycle;
pub mod provider;
pub mod telemetry;

pub use crate::errors::{DevcontainerError, Result};
