## Gateway [[gateway: Service]]

The target of `elided_ns`. It sits in the provider's `services` NAMESPACE, not at the
provider's workspace root, so `[[#qmd69_elision_provider::gateway]]` can only reach it if the
empty middle segment elides the namespace rather than asserting an empty one.

- runtime: Rust
- owner: qmd69_elision_provider
- where: provider services namespace
