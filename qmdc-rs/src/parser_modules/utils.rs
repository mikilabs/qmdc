//! Utility functions: regex helpers, RNG for ID generation

use regex::Regex;
use std::sync::OnceLock;

// ============================================================================
// Regex helpers (compiled once, cached)
// ============================================================================

pub fn re_special() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^\w\s-]").unwrap())
}

pub fn re_spaces() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[\s-]+").unwrap())
}

pub fn re_double_brackets() -> &'static Regex {
    // Balanced-ish match allowing nested brackets inside: [[ ... ]]
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[((?:[^\[\]]|\[[^\]]*\])*)\]\]").unwrap())
}

pub fn re_field_kv() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$").unwrap())
}

pub fn re_field_check() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*-?\s*[a-zA-Z_][a-zA-Z0-9_]*\s*:").unwrap())
}

// ============================================================================
// Simple RNG (Linear Congruential Generator)
// Same algorithm as TypeScript/Python implementations for cross-platform consistency
// ============================================================================

pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_float(&mut self) -> f64 {
        // Same LCG as Python/TypeScript
        self.state = (self.state.wrapping_mul(1664525).wrapping_add(1013904223)) % 4294967296;
        self.state as f64 / 4294967296.0
    }

    pub fn gen_id(&mut self) -> String {
        const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let mut result = String::with_capacity(6);
        for _ in 0..6 {
            let idx = (self.next_float() * CHARS.len() as f64) as usize;
            result.push(CHARS[idx] as char);
        }
        format!("object_{}", result)
    }

    pub fn gen_doc_id(&mut self) -> String {
        let obj_id = self.gen_id();
        // Convert object_xyz to doc_xyz
        format!("doc_{}", &obj_id[7..])
    }
}

// ============================================================================
// String utilities
// ============================================================================

/// Convert text to snake_case for auto-generated IDs
pub fn snake_case(text: &str, rng: &mut SimpleRng) -> String {
    // Remove special chars, replace spaces/dashes with underscore
    let cleaned = re_special().replace_all(text, "");
    let cleaned = re_spaces().replace_all(&cleaned, "_");

    let result = cleaned.to_lowercase();
    let result = result.trim_matches('_');

    if result.is_empty() {
        rng.gen_id()
    } else {
        result.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_matches_python() {
        let mut rng = SimpleRng::new(666);
        let id = rng.gen_doc_id();
        // Python/TS with seed 666 produces "doc_ry4ljv"
        assert_eq!(id, "doc_ry4ljv");
    }

    #[test]
    fn test_snake_case() {
        let mut rng = SimpleRng::new(42);
        assert_eq!(snake_case("Hello World", &mut rng), "hello_world");
        assert_eq!(snake_case("Some-Thing", &mut rng), "some_thing");
        assert_eq!(snake_case("Already_Snake", &mut rng), "already_snake");
    }
}
