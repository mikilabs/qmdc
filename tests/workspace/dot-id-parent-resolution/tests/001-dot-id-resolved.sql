-- Test: dot-ID object resolves parent from another file
SELECT __id, __parent
FROM objects
WHERE __id = 'my_service.operations'
