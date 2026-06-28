-- Check that included objects are present in the database
SELECT __id, __kind, __file
FROM objects
WHERE __id IN ('#IncludedQuery', '#workspace-a', '#ns-a', '#ServiceA')
ORDER BY __id;

