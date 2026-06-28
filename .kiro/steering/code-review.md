---
name: reviewing-code
description: Review code for quality, maintainability, and correctness. Use when reviewing pull requests, evaluating code changes, or providing feedback on implementations. Focuses on API design, patterns, and actionable feedback.
inclusion: manual
---

# Code Review

## Philosophy

Code review maintains a healthy codebase while helping contributors succeed. The burden of proof is on the PR to demonstrate it adds value. Your job is to help it get there through actionable feedback.

**Critical**: A perfectly written PR that adds unwanted functionality must still be rejected. The code must advance the codebase in the intended direction. When rejecting, provide clear guidance on how to align with project goals.

Be friendly and welcoming while maintaining high standards. Call out what works well. When code needs improvement, be specific about why and how to fix it.

## What to Focus On

### Does this advance the codebase correctly?

Even perfect code for unwanted features should be rejected.

### API design and naming

Identify confusing patterns or non-idiomatic code:
- Parameter values that contradict defaults
- Mutable default arguments
- Unclear naming that will confuse future readers
- Inconsistent patterns with the rest of the codebase

### Specific improvements

Provide actionable feedback, not generic observations.

### User ergonomics

Think about the API from a user's perspective. Is it intuitive? What's the learning curve?

## For Agent Reviewers

1. **Read the full context**: Examine related files, tests, and documentation before reviewing
2. **Check against established patterns**: Look for consistency with codebase conventions
3. **Verify functionality claims**: Understand what the code actually does, not just what it claims
4. **Consider edge cases**: Think through error conditions and boundary scenarios

## Blocking Errors

The following categories are **blocking** — the review MUST request changes (not just note them) if any are found. Do not approve code with these issues.

### 1. Code duplication that deepens tech debt

Copy-pasted logic between functions that should share a common implementation is a blocking error. Examples:
- Identical rebuild/serialization logic duplicated across multiple functions (e.g. `_rebuild_object_to_lines` and `rebuild` having the same branch)
- The same parsing logic repeated instead of extracted into a shared helper

If the duplication already exists in the codebase for other cases, the PR must either:
- Refactor the existing duplication while adding the new case, OR
- At minimum, add a `# TODO: deduplicate with <other_function>` comment and create a tracking issue

Simply matching existing bad patterns is not acceptable — the codebase should get better, not worse.

### 2. Cross-parser behavioral divergence

This project has three parser implementations (Python, TypeScript, Rust) that must produce identical output for identical input. A blocking error occurs when:
- One parser uses a different algorithm or code path than the others for the same feature (e.g. one hand-parses raw text while others use the shared field parser)
- Edge cases would produce different results across parsers (e.g. markdown formatting in values, whitespace handling)
- A feature relies on parser-specific internals that aren't guaranteed to behave the same way

The review must verify that all three parsers handle the same edge cases identically. If the implementations diverge in mechanism, there must be tests that exercise the divergent paths and prove they converge on output.

### 3. Inconsistent implementation strategies across parsers

When adding a feature to all three parsers, the implementation approach should be consistent unless there's a documented reason for divergence. Blocking examples:
- One parser reuses existing infrastructure (e.g. `parse_fields_from_list`) while another reimplements the logic from scratch
- One parser handles an edge case (e.g. multiline values) through a completely different mechanism than the others
- Implicit behavioral contracts that happen to work but for different reasons across parsers (e.g. empty value → empty string via `None` coercion in one parser vs. string slicing in another)

The fix is either: align the implementations, or add explicit tests that pin the shared behavior so future changes don't silently break one parser.

## Documentation Completeness

Any change that introduces new user-facing functionality, syntax, field types, CLI commands, or API changes **must** include corresponding documentation updates. This is a blocking requirement.

Check for:
- New field types or syntax → updated in `docs/format/` (heading-syntax, objects-and-fields, primitives, data-types as applicable)
- New CLI commands or flags → updated in `docs/architecture/cli-commands.qmd.md`
- New LSP features → updated in `docs/lsp/`
- New parser behavior → updated in `docs/parsers/`
- Changes to validation rules → updated in `docs/format/validation-errors.qmd.md`

If documentation is present but incomplete (e.g. a new field type is added to one doc page but not all relevant pages), flag the specific missing locations.

## What to Avoid

- Generic feedback without specifics
- Hypothetical problems unlikely to occur
- Nitpicking organizational choices without strong reason
- Summarizing what the PR already describes
- Star ratings or excessive emojis
- Bikeshedding style preferences when functionality is correct
- Requesting changes without suggesting solutions
- Focusing on personal coding style over project conventions

## Tone

- Acknowledge good decisions: "This API design is clean"
- Be direct but respectful
- Explain impact: "This will confuse users because..."
- Remember: Someone else maintains this code forever

## Decision Framework

Before approving, ask:

1. Does this PR achieve its stated purpose?
2. Is that purpose aligned with where the codebase should go?
3. Would I be comfortable maintaining this code?
4. Have I actually understood what it does, not just what it claims?
5. Does this change introduce technical debt?
6. Are all three parsers consistent in behavior AND implementation approach?
7. Is the documentation complete for any new functionality?

If something needs work, your review should help it get there through specific, actionable feedback. If it's solving the wrong problem, say so clearly.

## Comment Examples

**Good comments:**

| Instead of | Write |
|------------|-------|
| "Add more tests" | "The `handle_timeout` method needs tests for the edge case where timeout=0" |
| "This API is confusing" | "The parameter name `data` is ambiguous - consider `message_content` to match the MCP specification" |
| "This could be better" | "This approach works but creates a circular dependency. Consider moving the validation to `utils/validators.py`" |
| "Might diverge" | "BLOCKING: Rust hand-parses raw text while Python/TS use `parse_fields_from_list`. Add a test with markdown formatting in map values to prove they converge." |
| "Docs look fine" | "New `map` field type is in heading-syntax and primitives but missing from `data-types.qmd.md`" |

## Checklist

Before approving, verify:

- [ ] All required development workflow steps completed (uv sync, prek, pytest)
- [ ] Changes align with repository patterns and conventions
- [ ] API changes are documented and backwards-compatible where possible
- [ ] Error handling follows project patterns (specific exception types)
- [ ] Tests cover new functionality and edge cases
- [ ] The change advances the codebase in the intended direction
- [ ] No code duplication introduced (or existing duplication addressed)
- [ ] All three parsers use consistent implementation strategies
- [ ] Cross-parser output is identical for all edge cases
- [ ] Documentation is complete for all new user-facing functionality
