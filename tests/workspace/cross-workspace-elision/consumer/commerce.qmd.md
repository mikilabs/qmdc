## Commerce [[commerce: Project]]

`elided_ns` names the provider workspace and ELIDES the namespace with an empty middle
segment (operator decision, 2026-09-23). Its target sits in the provider's `services`
namespace, not at the provider's root, so the reference can only reach it if `::` elides
rather than asserting an empty namespace.

It must reach `qmd69_elision_provider:services:gateway` and must NOT bind to the consumer's
own root-level `gateway`, which shares the id. Two assertions in one edge: the qualifier
outranks a same-named local object, and the elided namespace still finds the target.

- elided_ns: [[#qmd69_elision_provider::gateway]]
