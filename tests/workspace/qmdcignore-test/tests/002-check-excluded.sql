-- Check that ignored objects are absent from the database
SELECT __id, __kind, __file
FROM objects
WHERE __id IN ('#ExcludedQuery', '#TempQuery', '#workspace-b', '#ns-b', '#ServiceB')
ORDER BY __id;

