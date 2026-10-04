// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Composable development tasks with typed arguments and shared context.
//!
//! Task attributes generate a sibling `function_task()` descriptor while
//! preserving the original function for ordinary Rust calls. Descriptors contain
//! their compile-time command names. Collect them with [`Registry::discover`]
//! or register them explicitly, then call [`Registry::run`] from your task binary.
extern crate self as bake;

mod arguments;
mod context;
mod error;
mod output;
mod registry;
mod task;
mod task_name;

#[doc(hidden)]
pub mod __private {
    pub use crate::registry::{TASK_REGISTRATIONS, TaskRegistration};
    pub use crate::task_name::TaskName;
    pub use linkme;
}

pub use arguments::{Arguments, Parameter};
pub use bake_macros::task;
pub use context::Context;
pub use error::{Error, Result};
pub use output::Format;
pub use registry::Registry;
pub use serde_json::Value;
pub use task::Task;

/// Convert a task's output into the shared, serializable result format.
pub fn value(output: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(output).map_err(Error::from)
}
