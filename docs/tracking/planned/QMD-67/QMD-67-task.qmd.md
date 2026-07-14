# QMD-67: duplicate_id ignores namespace for top-level ids

## Same top-level id in different namespaces is falsely flagged as duplicate_id [[qmd67_ns_dup: Bug]]

Workspace validation reports `duplicate_id` for two objects that share the same
top-level id but live in different namespaces. Namespaces scope object identity, but
the workspace validator keys uniqueness on the bare id alone and ignores the
namespace, producing false positives across namespaces.

- status: triage_review
- priority: high
- category: parser

### Reproduction [[qmd67_repro: text]]

- about: [[#qmd67_ns_dup]]

Two sibling namespaces, each defining a top-level object with the same id:

```text
ws/readme.qmd.md          # [[ws: __Workspace]]
ws/r1/readme.qmd.md       # [[r1: __Namespace]]  +  ## Foo [[foo: Thing]]
ws/r2/readme.qmd.md       # [[r2: __Namespace]]  +  ## Foo [[foo: Thing]]
```

`qmdc workspace validate ws` returns a `duplicate_id` error for `foo`, even though
the two objects have distinct `__namespace` values (`r1` and `r2`). It fires for both
flat namespaces (directly under root) and nested namespaces.

### Expected vs Actual [[qmd67_expected: text]]

- about: [[#qmd67_ns_dup]]

Expected: no error — `r1:foo` and `r2:foo` are distinct identities, disambiguated by
namespace and addressable through explicit cross-namespace references such as
`[[#r2:foo]]`.

Actual: `duplicate_id` error, "Duplicate ID 'foo' found in multiple files".

Contrast: the same leaf name reused as a dot-child under different parents
(`auth_svc.config` vs `payment_svc.config`, test `hierarchical-ids-no-dup`) is
correctly accepted, because the full dot-path id differs. Only namespace scoping is
broken.

### Root Cause [[qmd67_rootcause: text]]

- about: [[#qmd67_ns_dup]]

In `qmdc-rs/src/workspace.rs` the `objects_by_id` index is keyed by the bare `id`
string. The namespace is captured into the tuple (third element) but is not part of
the uniqueness key; the duplicate check then flags any id appearing in more than one
file, regardless of namespace. The same logic exists in
`qmdc-py/qmdc/workspace.py` and `qmdc-ts/src/workspace.ts`. Duplicate validation
should group by `(namespace, id)`. Kind-specific semantics are intentionally out of
scope because Kind support is planned for removal separately.

### Goals [[goals: [Goal]]]

#### A1: Namespace-scope duplicate detection in Rust [[qmd67_goal_a1]]

Change duplicate validation in `qmdc-rs/src/workspace.rs` from bare `id` grouping to
`(namespace, id)` so identical top-level ids in different namespaces are not flagged.
Preserve genuine within-namespace duplicate detection.

- group: A_fix
- done: false

#### A2: Namespace-scope duplicate detection in Python [[qmd67_goal_a2]]

Apply the same `(namespace, id)` duplicate grouping in
`qmdc-py/qmdc/workspace.py`.

- group: A_fix
- done: false

#### A3: Namespace-scope duplicate detection in TypeScript [[qmd67_goal_a3]]

Apply the same `(namespace, id)` duplicate grouping in
`qmdc-ts/src/workspace.ts` so all parser implementations agree.

- group: A_fix
- done: false

#### B1: Data-driven test — same top-level id across namespaces is valid [[qmd67_goal_b1]]

Keep the existing `duplicate-id-namespace-scoping` fixture unchanged as the flat
namespace regression. Add one minimal shared fixture for nested namespaces because
that case is not covered elsewhere. Do not add Kind-specific variants.

- group: B_tests
- done: false

#### B2: Verify existing coverage — real duplicates still caught [[qmd67_goal_b2]]

Use the existing `validation-parser-consistency` assertions to verify objects with
the same id in the same namespace still produce `duplicate_id`; do not add a
duplicative fixture.

- group: B_tests
- done: false
