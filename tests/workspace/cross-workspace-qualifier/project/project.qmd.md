## Commerce [[commerce: Project]]

`includes` names repo_a explicitly, so it must bind to repo_a's payments_api and
never to the identically-named object in repo_b. `bogus` names a workspace that does
not exist, so it must not resolve at all rather than falling back to any workspace
that happens to hold the id. `root_ns` targets an object in repo_a's root namespace, using
the `workspace::id` form where the empty middle segment states that the namespace is the
workspace root.

`bare_unique` and `bare_duplicated` name no workspace at all. Per the operator decision of
2026-09-23 a cross-workspace reference must be qualified, so neither may resolve — not even
`bare_unique`, whose target `billing` is held by exactly one workspace in this container.

`elided_ambiguous` uses the `workspace::id` form, where the empty middle segment ELIDES the
namespace rather than asserting an empty one (operator decision, 2026-09-23). It has two
candidates in repo_b — root `ledger` and `services:ledger` — so it must report ambiguity and
bind nothing. The positive half of that rule needs a source-workspace shadow to fail
deterministically, so it lives in its own fixture, `cross-workspace-elision`.

No reference below carries a Kind segment: the canonical form is `workspace:namespace:id`
with an optional `.field` suffix, matching `__global_id`.

- includes: [[#qmd69_repo_a:services:payments_api]]
- bogus: [[#qmd69_no_such_ws:services:payments_api]]
- root_ns: [[#qmd69_repo_a::billing]]
- bare_unique: [[#billing]]
- bare_duplicated: [[#payments_api]]
- elided_ambiguous: [[#qmd69_repo_b::ledger]]
