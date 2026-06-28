-- QMD-34: Objects with same __id in different workspaces should both exist
SELECT __id, __workspace, __kind
FROM objects 
WHERE __id = 'app_config'
ORDER BY __workspace

