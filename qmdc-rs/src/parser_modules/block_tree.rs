//! Minimal BlockTree utilities.
//!
//! Production goal: keep only what the current parser uses
//! (raw source + offset<->line helpers), without carrying unused IR code.

use pulldown_cmark::Event;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct BlockTree {
    pub source: String,
    line_starts: Vec<usize>,
}

impl BlockTree {
    pub fn new(source: String) -> Self {
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        Self {
            source,
            line_starts,
        }
    }

    /// Convert byte offset to 1-based line number.
    pub fn offset_to_line(&self, offset: usize) -> u32 {
        if self.line_starts.is_empty() {
            return 1;
        }

        match self.line_starts.binary_search(&offset) {
            Ok(idx) => (idx + 1) as u32,
            Err(idx) => idx.max(1) as u32,
        }
    }

    /// Find the start byte offset of the line containing `offset`.
    pub fn line_start_offset(&self, offset: usize) -> usize {
        if self.line_starts.is_empty() {
            return 0;
        }

        match self.line_starts.binary_search(&offset) {
            Ok(idx) => self.line_starts[idx],
            Err(idx) => {
                if idx == 0 {
                    0
                } else {
                    self.line_starts[idx - 1]
                }
            }
        }
    }
}

/// Build BlockTree from an already-collected pulldown-cmark event stream.
///
/// We keep this signature to avoid re-parsing markdown twice; the current
/// production parser only needs the original `source` and line offsets.
pub fn build_block_tree_from_events(source: &str, _events: &[(Event, Range<usize>)]) -> BlockTree {
    BlockTree::new(source.to_string())
}
