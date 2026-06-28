//! Shared level-based nesting algorithm — the single source of truth for turning a flat,
//! line-ordered list of items (each with a `__level`) into a parent/child hierarchy.
//!
//! Used by both the LSP `document_symbol` handler and the Core `outline` op so the
//! outline/symbol nesting is computed identically for editors and agents.

/// Given the levels of a flat, line-ordered list, return for each index the index of its
/// parent (the nearest preceding item with a strictly lower level), or `None` for roots.
///
/// This is the canonical nesting rule: an item at level N+1 following an item at level N
/// becomes its child.
pub fn parent_map_by_level<T: PartialOrd>(levels: &[T]) -> Vec<Option<usize>> {
    let mut parent: Vec<Option<usize>> = vec![None; levels.len()];
    for i in 0..levels.len() {
        for j in (0..i).rev() {
            if levels[j] < levels[i] {
                parent[i] = Some(j);
                break;
            }
        }
    }
    parent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_list_all_roots() {
        assert_eq!(parent_map_by_level(&[1, 1, 1]), vec![None, None, None]);
    }

    #[test]
    fn nested_children() {
        // levels: 1, 2, 2, 1  → second & third are children of first; fourth is a root
        assert_eq!(
            parent_map_by_level(&[1, 2, 2, 1]),
            vec![None, Some(0), Some(0), None]
        );
    }

    #[test]
    fn deep_nesting() {
        // 1, 2, 3 → chain
        assert_eq!(
            parent_map_by_level(&[1, 2, 3]),
            vec![None, Some(0), Some(1)]
        );
    }
}
