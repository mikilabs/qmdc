-- Test: whole-number float literals serialize to a canonical `X.0` form,
-- byte-identical across all three parsers, regardless of how many trailing
-- zeros the source had (1.0, 1.00, 2.000 all -> X.0) and regardless of nesting
-- (scalars AND array elements). JS collapses 1.0->1 and over-preserves 1.00,
-- so the TS parser must canonicalize when building the data column.
SELECT __id, data
FROM objects
WHERE __id = 'float_probe'
