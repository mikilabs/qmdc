//! `[Core]` stderr side-channel logging (cross-cutting §7, A09 auditability).
//!
//! Operational logs go to stderr so they never pollute the agent-facing stdout payload.
//! The `security-rejection` category is **always** logged (A09). No new dependencies;
//! uses `eprintln!` matching the existing `[LSP]`/`[qmdc]`/`[LSP Tree]` convention.

/// Event categories for Core logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCategory {
    /// Which root a path resolved to, resolution misses.
    Resolution,
    /// The query executed and whether it was admitted/stripped.
    Query,
    /// An INV-1/INV-2/INV-3 fail-closed denial. **Always logged** (A09).
    SecurityRejection,
}

impl EventCategory {
    /// String tag for the log line.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resolution => "resolution",
            Self::Query => "query",
            Self::SecurityRejection => "security-rejection",
        }
    }
}

/// Severity levels for Core log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Normal resolution/query activity.
    Info,
    /// Recoverable fallback, near-miss.
    Warning,
    /// A fail-closed rejection — always logged for auditability.
    Security,
}

impl Severity {
    /// String tag for the log line.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Security => "SECURITY",
        }
    }
}

/// Log a Core event to stderr.
///
/// Format: `[Core][{severity}][{category}] {message}`
///
/// `security-rejection` events are always emitted regardless of any future log-level control.
pub fn core_log(category: EventCategory, severity: Severity, message: &str) {
    eprintln!(
        "[Core][{}][{}] {}",
        severity.as_str(),
        category.as_str(),
        message
    );
}
