//! Build-embedded QMDC agent guide (FR-19).
//!
//! Single source of truth for the guide content, shared by BOTH transports:
//! - the `qmdc://guide` MCP **resource** (`resources/read`), and
//! - the `get_guide` MCP **tool** (`tools/call`) — added so clients that bridge only
//!   MCP *tools* (not *resources*) can still reach the guide.
//!
//! The guide is compiled into the binary via `include_str!` — no runtime file read, and
//! the content is version-locked to the binary.
//!
//! The canonical source is `docs/guides/qmdc-guide.qmd.md`; a vendored copy lives at
//! `qmdc-rs/src/qmdc-guide.qmd.md` so the crate is self-contained and publishable to
//! crates.io. Keep them in sync with `make guide-sync` (mirrors the mermaid-core sync).

/// The QMDC agent guide markdown, embedded at build time.
pub const GUIDE_CONTENT: &str = include_str!("../qmdc-guide.qmd.md");
