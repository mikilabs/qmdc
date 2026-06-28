-- Test: child objects have __parent set to their parent's ID
SELECT child.__id, child.__parent, parent.__id as parent_id
FROM objects child
JOIN objects parent ON child.__parent = parent.__id
  AND child.__workspace = parent.__workspace
WHERE child.__parent IS NOT NULL
ORDER BY child.__id
