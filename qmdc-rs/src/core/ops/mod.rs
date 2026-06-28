//! Core operations module — pure-functional ops over `&ResolvedIndex`.
//!
//! Each op returns `Result<Value, Value>` where both sides are in-band envelopes.

pub mod describe;
pub mod describe_metamodel;
pub mod dump;
pub mod find_path;
pub mod locate;
pub mod outline;
pub mod query;
pub mod references;
pub mod rename_plan;
pub mod search;
pub mod traverse;
pub mod tree;
pub mod validate;

pub use describe::describe;
pub use describe_metamodel::describe_metamodel;
pub use dump::dump;
pub use find_path::find_path;
pub use locate::locate;
pub use outline::outline;
pub use query::query;
pub use references::find_references;
pub use rename_plan::rename_plan;
pub use search::search;
pub use traverse::traverse;
pub use tree::tree;
pub use validate::validate;
