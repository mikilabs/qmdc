//! Parser modules: reusable components for QMDC parsing
//!
//! This module provides decomposed parsing functionality:
//! - `utils`: Regex helpers, RNG, string utilities
//! - `references`: [[#ref]] extraction and classification  
//! - `header`: Header parsing [[id:Kind]]
//! - `value_parser`: YAML/JSON value parsing
//! - `output`: OutputFormat and JSON building
//! - `block_tree`: BlockTree intermediate representation (Stage 2)

mod block_tree;
mod header;
mod output;
mod references;
mod utils;
mod value_parser;

// Re-export commonly used items
pub use block_tree::build_block_tree_from_events;
pub use header::parse_header;
pub use output::{build_from_map, OutputFormat};
pub use references::{extract_references_from_line, Reference};
pub use utils::{re_double_brackets, re_field_check, re_field_kv, SimpleRng};
pub use value_parser::parse_field_value;
