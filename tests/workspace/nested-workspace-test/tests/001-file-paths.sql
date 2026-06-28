-- Test that __file paths are relative to PROJECT ROOT, not workspace root
-- This is critical for extension.ts which uses project_root + __file
SELECT __id, __file FROM objects 
WHERE __id IN ('myproject', 'business', 'stakeholders', 'researcher')
ORDER BY __id
