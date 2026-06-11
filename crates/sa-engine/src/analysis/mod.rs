//! Analysis pipeline modules.
//!
//! Contains the graph helpers, lifecycle management (task creation/running/status),
//! result assembly, and the runtime trading graph execution engine.

pub mod diagnosis;
pub mod graph;
pub mod lifecycle;
pub mod result;
pub mod runtime;
