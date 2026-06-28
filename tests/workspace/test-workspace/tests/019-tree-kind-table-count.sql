-- Test: Count objects of Kind=Table
SELECT COUNT(*) as count FROM objects 
WHERE __kind = 'Table'
