-- Test: Should load objects from both explicit workspaces AND orphan files
-- workspace-a: workspace_a, service_a = 2 objects
-- workspace-b: workspace_b, service_b = 2 objects  
-- orphans: queries, get_all, user_model, order_model = 4 objects
-- Total = 8 objects
SELECT COUNT(*) as count FROM objects 
WHERE __kind NOT IN ('__Document', '__TextBlock')

