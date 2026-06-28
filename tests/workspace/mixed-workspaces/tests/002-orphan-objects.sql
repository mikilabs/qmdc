-- Test: Orphan files should be loaded (not ignored)
SELECT __id FROM objects 
WHERE __id IN ('queries', 'get_all', 'user_model', 'order_model')
ORDER BY __id

