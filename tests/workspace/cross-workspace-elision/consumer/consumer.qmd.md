## Gateway [[gateway: Service]]

A LOCAL object at the consumer workspace's root, deliberately sharing its id with the
`gateway` in the provider workspace's `services` namespace.

Its only job is to make the failure of `elided_ns` DETERMINISTIC rather than lucky. Edge
resolution starts from the SOURCE object's own workspace and namespace, so today
`[[#qmd69_elision_provider::gateway]]` matches this object on the first try and never consults
the workspace qualifier — identically in all three implementations, with no arbitrary
`LIMIT 1` ordering involved.

An explicit workspace qualifier must outrank a same-named local object.

- runtime: Python
- owner: qmd69_elision_consumer
- where: consumer workspace root
