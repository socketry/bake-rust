//! Composable development tasks with typed arguments and shared context.
//!
//! Task attributes generate a sibling `function_task()` descriptor while
//! preserving the original function for ordinary Rust calls. Register those
//! descriptors explicitly and call [`Registry::run`] from your task binary.

extern crate self as bake;

mod arguments;
mod context;
mod error;
mod registry;
mod task;

pub use arguments::{Arguments, Parameter};
pub use context::Context;
pub use error::{Error, Result};
pub use registry::Registry;
pub use serde_json::Value;
pub use socketry_bake_macros::task;
pub use task::Task;

/// Convert a task's output into the shared, serializable result format.
pub fn value(output: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(output).map_err(Error::from)
}
