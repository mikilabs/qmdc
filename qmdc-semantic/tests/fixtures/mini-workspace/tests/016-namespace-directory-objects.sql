-- Test that namespace shows objects from all files in its directory
-- This tests the bug fix where namespace showed 0 objects when objects 
-- were in other files within the same directory

-- Get namespace 'storage' (in storage/readme.qmd.md)
-- and count objects in its directory (storage/*)
SELECT COUNT(*) as object_count
FROM objects 
WHERE __kind NOT IN ('__TextBlock', '__Workspace', '__Document', '__Namespace')
  AND __file LIKE 'storage/%'
