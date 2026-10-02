# QMD-78: Findings

## What was measured, and what was not [[qmd78_finding_measurement: Finding]]

The divergence was found by reading the three ignore implementations while closing QMD-77, not by
running them: on APFS the filename cannot be created, so no local run can reach it. Python's matcher
receives the path as `str` decoded with `surrogateescape`, which round-trips the invalid byte, while
the Rust and TypeScript matchers see a string in which that byte has already become U+FFFD.

So the finding is a code reading, not a measurement, and the first step of the work is to turn it into
one on Linux ext4: create the name, apply a pattern that should match it, and record what each parser
answers. The expected shape is that Python matches and the other two do not, but that has to be seen
rather than assumed — an intermediate layer could normalise before either matcher is reached.

- category: parser
- related_to: [[#qmd78_non_utf8_path]]
- solution: Reproduce on Linux ext4 first; the three-way split is read from the code, not yet observed.
