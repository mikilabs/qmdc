---
name: reviewing-code
description: Review code for quality, maintainability, and correctness. Use when reviewing pull requests, evaluating code changes, or providing feedback on implementations. Invokes parallel subagent reviewers from different perspectives.
inclusion: manual
---

# Code Review

## Philosophy

Code review maintains a healthy codebase while helping contributors succeed. The burden of proof is on the PR to demonstrate it adds value. Your job is to help it get there through actionable feedback.

**Critical**: A perfectly written PR that adds unwanted functionality must still be rejected. The code must advance the codebase in the intended direction. When rejecting, provide clear guidance on how to align with project goals.

Be friendly and welcoming while maintaining high standards. Call out what works well. When code needs improvement, be specific about why and how to fix it.

## Multi-Perspective Review Process

When reviewing code (especially before commit/deploy), invoke **parallel subagents** — each reviewing from a different perspective. This catches issues that a single-pass review misses.

### How to run

1. Get the staged diff: `git diff --cached > /tmp/staged_diff.txt`
2. Invoke 3-4 subagents in parallel, each with a different persona and focus area
3. Collect results, deduplicate, and present a unified issue list
4. Fix blockers before committing

### Reviewer Perspectives

Each subagent gets the diff + relevant source files as context. Each has a distinct focus:

#### 1. Security & Sandboxing Reviewer

Focus: path traversal, credential leaks, injection, sandbox escapes, JWT handling.

Prompt seed:
> You are a security-focused code reviewer. Look for: path traversal (../), credential exposure in logs/errors, prompt injection vectors, sandbox escape (can tools access files outside workspace?), JWT expiry during long operations, SSRF via user-controlled URLs. Check that all file paths go through realpath + containment checks. Check that secrets never appear in tool return values or error messages.

#### 2. Observability & Cost Reviewer

Focus: Langfuse trace correlation, Sentry context propagation, cost tracking accuracy, log completeness.

Prompt seed:
> You are an observability reviewer. Check: Do sub-agents propagate trace_attributes (langfuse.user.id, langfuse.session.id, trace_id)? Do all sentry_sdk.capture_exception calls have proper scope (user_id, session_id)? Is token usage logged for ALL model calls (including sub-agents)? Is the pricing table complete for all models used? Are structured log fields consistent (same keys as existing code)? Would you be able to debug a production issue using only these logs?

#### 3. Reliability & Edge Cases Reviewer

Focus: timeouts, retries, error propagation, asyncio correctness, resource leaks.

Prompt seed:
> You are a reliability reviewer. Look for: What happens when external services fail (ElevenLabs timeout, FM 500, Bedrock throttle)? Are there unbounded loops or missing max-iteration caps? Can asyncio coroutines starve each other? Are HTTP connections properly closed on error paths? What happens with 0-byte files, empty responses, malformed JSON? Does the code handle the case where workspace_dir doesn't exist? Are there race conditions between concurrent requests sharing the same session?

#### 4. UX & Contract Reviewer

Focus: user-visible behavior, API contracts, backward compatibility, SKILL.md accuracy.

Prompt seed:
> You are a UX and contract reviewer. Check: Does the user see raw error messages or implementation details? Are tool return formats stable (other code parses them)? Does SKILL.md accurately describe what the tools actually do? Are there breaking changes to existing tool schemas? Will the LLM reliably call the tools in the right order given the instructions? Could the LLM hallucinate file_ids or paths? Is the speaker_map format documented clearly enough for the LLM to construct it correctly?

#### 5. Code Quality & Maintainability Reviewer

Focus: duplication, naming, separation of concerns, testability, dead code, consistency with codebase patterns.

Prompt seed:
> You are a code quality reviewer. Look for: duplicated logic that should be extracted into a helper, inconsistent naming vs the rest of the codebase, functions doing too many things (>1 responsibility), dead code or unused imports, magic numbers without constants, overly complex conditionals that could be simplified, test code that's tautological (tests that can't fail), missing type hints on public APIs, docstrings that lie about what the code does. Compare patterns with existing code in the same directory — new code should match established conventions.

#### 6. Cross-Language Consistency Reviewer

**Activation condition:** Only invoke this reviewer when the diff touches multiple parser implementations (any combination of `qmdc-py`, `qmdc-ts`, `qmdc-rs`, or the LSP server). If the change is in a single implementation only, skip this reviewer entirely.

Focus: behavioral parity across TypeScript, Rust, and Python parser implementations; CLI command equivalence; LSP server alignment with core parser semantics.

Prompt seed:
> You are a cross-language consistency reviewer for a multi-implementation project (Python `qmdc-py`, TypeScript `qmdc-ts`, Rust `qmdc-rs`, plus an LSP server). All implementations must produce identical results for the same input. This review only fires when the diff spans multiple implementations. Check: Are syntactic constructs (object detection, field parsing, array handling, reference resolution, text field detection, auto-detection rules) handled identically across the touched implementations? Do CLI commands (`parse`, `rebuild`, `lint`, `workspace validate`, `query`) produce the same output format and honor the same flags? Does the LSP server's behavior (diagnostics, completions, hover) reflect the same parsing semantics as the standalone parsers? Look for: divergent edge-case handling (e.g., one parser treats a construct as a text field while another treats it as an object), inconsistent error messages or validation error types, different default behaviors for the same flag, round-trip differences (parse→rebuild produces different QMD.md across implementations), logic that was implemented differently rather than equivalently across the languages in this diff.

### Output Format

Each subagent produces:
```
Verdict: clean | issues_found

Issues (if any):
1. [SEVERITY] Category — description
2. ...
```

Severities: blocker (must fix), high (should fix), medium (consider fixing), low (note for future), info (observation).

### Consolidation

After all subagents return:
1. Deduplicate (same issue found by multiple reviewers = higher confidence)
2. Prioritize blockers and highs
3. Present unified table to the developer
4. Fix blockers, then commit

### Review Output Rules

**If the consolidated review contains more than 3 items to fix** (any severity except info):
1. Write the full review to a markdown file in the `reviews/` folder at workspace root
2. File naming: `{NN}-cr-{short-subject}.md` where:
   - `{NN}` is the next sequential number (zero-padded, e.g., `00`, `01`, `02`)
   - `cr` stands for "code review"
   - `{short-subject}` is a brief kebab-case identifier for the reviewed scope (e.g., `aaa-v1`, `adapter-billing`, `telegram-bot-refactor`)
3. Check existing files in `reviews/` to determine the next sequence number
4. **Questionnaire on top**: If any review items have ambiguity on how to tackle them (multiple valid approaches, tradeoffs to discuss, or decisions that depend on user preference), place a questionnaire section at the very top of the file before the review body. Format:

```markdown
# Questions Before Fixing

> These items have multiple valid approaches. Please answer before I proceed.

## Q1: [Short question title]
[Context and options]
- **A)** Option description
- **B)** Option description

## Q2: ...
```

5. After the questionnaire (or at the top if no questions), include the full review with all tables, descriptions, and fix suggestions
6. In the chat response, provide a brief summary (blockers count, total issues) and reference the file path

**If the review has 3 or fewer items**: present inline in chat as usual (no file needed).

## What to Focus On (Single-Pass Fallback)

If subagents are unavailable, review manually with these priorities:

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

If something needs work, your review should help it get there through specific, actionable feedback. If it's solving the wrong problem, say so clearly.

## Comment Examples

**Good comments:**

| Instead of | Write |
|------------|-------|
| "Add more tests" | "The `handle_timeout` method needs tests for the edge case where timeout=0" |
| "This API is confusing" | "The parameter name `data` is ambiguous - consider `message_content` to match the MCP specification" |
| "This could be better" | "This approach works but creates a circular dependency. Consider moving the validation to `utils/validators.py`" |

## Checklist

Before approving, verify (examples — extend with project-specific checks as needed):

- [ ] All required development workflow steps completed (uv sync, prek, pytest)
- [ ] Changes align with repository patterns and conventions
- [ ] API changes are documented and backwards-compatible where possible
- [ ] Error handling follows project patterns (specific exception types)
- [ ] Tests cover new functionality and edge cases
- [ ] The change advances the codebase in the intended direction

## Release Review Checklist

Additional checks for multi-component releases. These are permanent rules, not suggestions.

### Deployment

- [ ] `make deploy` is the single command. No manual steps, no forgotten Lambda rebuilds.
- [ ] Every new env var exists in ALL four places: `src/config.py`, `.env.example`, `.env.prod`, `cross-cutting.qmd.md`
- [ ] Every new Secrets Manager secret has IAM grants on the EC2 role (CDK AgentStack), not just the resource stack
- [ ] EC2 instance profile is CDK-managed. Manual IAM roles are forbidden.

### User-facing error messages

- [ ] Exception text NEVER reaches the user. Sanitized messages only.
- [ ] Every caught exception that indicates a real failure calls `sentry_sdk.capture_exception(e)`. If it's worth logging, it's worth sending to Sentry.

### Sandbox / Code Interpreter

- [ ] Skill scripts use RELATIVE paths. Absolute host paths are forbidden in system prompt and SKILL.md.
- [ ] Sandbox file edits use `code_interpreter`, never `editor` tool
- [ ] Agent does not mention Python, XML, zipfile, or any implementation detail to the user
- [ ] Heavy validation (XSD, schema checks) is off by default. Opt-in only.

### Observability (sub-agents & tools)

- [ ] Sub-agents pass `trace_attributes` for Langfuse correlation (user_id, session_id, trace_id)
- [ ] Sub-agent token usage is logged with model_id and cost_usd
- [ ] All models used have entries in `src/agent/pricing.py`
- [ ] sentry_sdk.capture_exception is called in all error paths (not just logged)

### Code ↔ docs

- [ ] CloudWatch log group paths in scripts match CDK-created paths exactly
- [ ] File protocol attributes in code match the contract spec
- [ ] Dead config (env vars set but never read) does not exist
- [ ] SKILL.md instructions match actual tool behavior and return formats

### Tests

- [ ] Assertions prove the specific feature, not just "something happened"
- [ ] Falsy checks on `bytes` use `is None`, not `not value` (empty bytes is valid)
- [ ] File transfer paths handle zero-byte files
- [ ] Live tests verify file delivery (not just text response)
- [ ] Sub-agent behavior is tested with mocked Agent class (not real LLM calls)
